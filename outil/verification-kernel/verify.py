#!/usr/bin/env python3
"""Reproducible Verus check of the production policy, with optional mutants.

No tool is downloaded or installed by this program. A missing verifier, a
wrong release, a timeout or a malformed result is an infrastructure error.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import time


RELEASE = "0.2026.10.26.d17fbd0"
SUMMARY = re.compile(r"verification results::\s*(\d+) verified,\s*(\d+) errors")
UNCHECKED = re.compile(
    r"\b(?:assume|admit|assume_specification)\s*[(!]"
    r"|#\s*\[\s*verifier\s*::\s*(?:external|external_body|external_fn_specification)\b"
)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def run_case(verus, source, output, name, expected, mutation=None):
    command = [verus, "--crate-type", "lib", "--crate-name", "verification_kernel", str(source)]
    started = time.monotonic()
    try:
        completed = subprocess.run(command, capture_output=True, text=True, timeout=60, check=False)
        log = completed.stdout + completed.stderr
        exit_code = completed.returncode
        infrastructure_error = None
    except subprocess.TimeoutExpired as error:
        log = f"Verifier exceeded 60 seconds: {error}\n"
        exit_code = None
        infrastructure_error = "timeout"
    log_path = output / f"{name}.log"
    log_path.write_text(log)
    summary = SUMMARY.search(log)
    verified = int(summary.group(1)) if summary else None
    errors = int(summary.group(2)) if summary else None
    if not summary and infrastructure_error is None:
        infrastructure_error = "verifier did not produce a verification summary"
    if expected == "verified":
        passed = exit_code == 0 and verified is not None and verified > 0 and errors == 0
    else:
        # A syntax/type error is not evidence that the specification caught a bug.
        passed = exit_code is not None and exit_code != 0 and errors is not None and errors > 0
    result = {
        "case": name,
        "expected": expected,
        "command": command,
        "source_sha256": digest(source.read_bytes()),
        "exit_code": exit_code,
        "verified_functions": verified,
        "verification_errors": errors,
        "infrastructure_error": infrastructure_error,
        "passed": passed and infrastructure_error is None,
        "elapsed_ms": round((time.monotonic() - started) * 1000),
        "log": str(log_path),
    }
    if mutation is not None:
        result["mutation"] = mutation
    print(f"{'PASS' if result['passed'] else 'FAIL'} {name}: {verified} verified, {errors} errors")
    return result


def mutations(source):
    changes = [
        (
            "stale-evidence",
            "(Match::Matches, Match::Matches, Freshness::Current))",
            "(Match::Matches, Match::Matches, Freshness::Current | Freshness::Stale))",
            "Accept stale evidence without changing its specification.",
        ),
        (
            "not-run-is-success",
            "Execution::NotRun | Execution::Ignored => Verdict::Incomplete,",
            "Execution::NotRun | Execution::Ignored => Verdict::Satisfied,",
            "Treat an unexecuted or ignored test as success.",
        ),
        (
            "aggregation-hides-pending",
            "if priority(left) >= priority(right) { left } else { right }",
            "if priority(left) >= priority(right) { left } else { left }",
            "Lose an unresolved right-hand obligation during aggregation.",
        ),
    ]
    for name, before, after, description in changes:
        if source.count(before) != 1:
            raise ValueError(f"Mutation {name}: expected exactly one production source match")
        yield name, source.replace(before, after), description


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--verus", default=os.environ.get("SOIREE_VERUS", "verus"))
    parser.add_argument("--mutations", action="store_true", help="Also require three real bad policies to fail verification")
    parser.add_argument("--output", type=Path, help="Report directory (default: soiree/target/verification-policy)")
    args = parser.parse_args()
    kernel = Path(__file__).resolve().parent
    repository = kernel.parent.parent
    output = (args.output or repository / "target" / "verification-policy").resolve()
    output.mkdir(parents=True, exist_ok=True)
    source_path = kernel / "src" / "lib.rs"
    source_bytes = source_path.read_bytes()
    source = source_bytes.decode()
    report = {
        "schema": 1,
        "property": "soiree.verification-policy",
        "method": "verus",
        "expected_verus_release": RELEASE,
        "source": str(source_path),
        "source_sha256": digest(source_bytes),
        "inputs": {
            str(path.relative_to(kernel)): digest(path.read_bytes())
            for path in [source_path, kernel / "Cargo.toml", kernel / "Cargo.lock"]
        },
        "runner_sha256": digest(Path(__file__).read_bytes()),
        "unchecked_kernel_annotations": [],
        "trust_boundary": [
            "The caller resolves applicability and constructs faithful Facts.",
            "The caller compares required methods, scopes and actual input fingerprints.",
            "The verifier, its solver, vstd, Rust compilation and execution platform are trusted.",
            "The theorem covers policy decisions, not game rules or the truth of external observations.",
        ],
        "cases": [],
        "passed": False,
    }
    try:
        unchecked = UNCHECKED.findall(source)
        if unchecked:
            report["unchecked_kernel_annotations"] = unchecked
            raise ValueError("Unchecked proof annotations are forbidden in the production kernel")
        verus = shutil.which(args.verus)
        if verus is None:
            raise ValueError(f"Verus not found: {args.verus}; set SOIREE_VERUS to the pinned binary")
        version = subprocess.run([verus, "--version"], capture_output=True, text=True, timeout=15, check=False)
        report["verus_version"] = version.stdout + version.stderr
        if version.returncode != 0 or not re.search(rf"Version:\s*{re.escape(RELEASE)}\b", report["verus_version"]):
            raise ValueError(f"Expected Verus {RELEASE}; see verus_version in the report")
        report["verus_binary_sha256"] = digest(Path(verus).read_bytes())
        report["cases"].append(run_case(verus, source_path, output, "production", "verified"))
        if report["cases"][0]["passed"] and args.mutations:
            # Mutate copies of the exact production source, keeping all contracts.
            with tempfile.TemporaryDirectory(prefix="soiree-policy-mutants-") as temporary:
                for name, changed, description in mutations(source):
                    path = Path(temporary) / name / "lib.rs"
                    path.parent.mkdir()
                    path.write_text(changed)
                    report["cases"].append(run_case(verus, path, output, name, "rejected", description))
        report["source_unchanged"] = source_bytes == source_path.read_bytes()
        report["inputs_unchanged"] = all(
            expected == digest((kernel / relative).read_bytes())
            for relative, expected in report["inputs"].items()
        ) and report["runner_sha256"] == digest(Path(__file__).read_bytes())
        report["passed"] = report["source_unchanged"] and report["inputs_unchanged"] and all(
            case["passed"] for case in report["cases"]
        )
    except (OSError, ValueError, subprocess.TimeoutExpired) as error:
        report["infrastructure_error"] = str(error)
        print(str(error), file=sys.stderr)
    finally:
        report_path = output / "report.json"
        report_path.write_text(json.dumps(report, indent=2) + "\n")
        print(f"Report: {report_path}")
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    sys.exit(main())
