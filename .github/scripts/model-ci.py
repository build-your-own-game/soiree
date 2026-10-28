"""Prevalidate the unpublished checkout, without asserting it is publishable.

No Cargo command sees the generated, potentially absent release tag: the local
fixture replaces its three shared dependencies first. The --published check is
then required to reject this same fixture, preserving the distribution gate.
"""
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import time

root = Path(__file__).resolve().parents[2]
parent = Path(os.environ["RUNNER_TEMP"]) / "soiree-model"
parent.mkdir()
subprocess.run(["soiree", "new", "ci-model", "--dans", str(parent)], check=True)
game = parent / "ci-model"
cached_target = Path(os.environ["RUNNER_TEMP"]) / "soiree-model-target"
if cached_target.exists():
    shutil.move(str(cached_target), str(game / "target"))
try:
    manifest = game / "Cargo.toml"
    text = manifest.read_text()
    for crate in ("core", "host", "web"):
        text, count = re.subn(
            rf'^salon-{crate} = .*$',
            f'salon-{crate} = {{ path = "{root / "salon" / "crates" / crate}" }}',
            text,
            flags=re.MULTILINE,
        )
        if count != 1:
            raise RuntimeError(f"expected one generated salon-{crate} dependency")
    manifest.write_text(text)
    if 'git = "https://github.com/build-your-own-game/soiree"' in text:
        raise RuntimeError("unpublished common dependency escaped the local fixture rewrite")
    if not (game / "verification.toml").is_file():
        raise RuntimeError("the generated game has no structured verification contract")
    subprocess.run(["git", "init", "-b", "main"], cwd=game, check=True)
    reports = game / "certification"
    reports.mkdir(exist_ok=True)
    with (reports / "plan.json").open("w") as plan:
        subprocess.run(["soiree", "contracts", "--write-view", str(game)], stdout=plan, check=True)
    measurements = []


    def run_phase(name, command, check=True, **options):
        started = time.monotonic()
        completed = subprocess.run(command, check=False, **options)
        measurements.append({
            "phase": name,
            "command": command,
            "exit_code": completed.returncode,
            "elapsed_seconds": round(time.monotonic() - started, 3),
        })
        report = game / "certification" / "model-ci.json"
        report.parent.mkdir(exist_ok=True)
        report.write_text(json.dumps({"schema": 1, "local_prevalidation": True, "phases": measurements}, indent=2) + "\n")
        if check:
            completed.check_returncode()
        return completed


    def require_fresh_selected_report(name, selection=None):
        report = json.loads((game / "certification" / name).read_text())
        if not report["evaluation"]["selected_scope_satisfied"]:
            raise RuntimeError(f"{name}: the requested scope was not satisfied")
        if not report["identity_before"] or report["identity_before"] != report["identity_after"]:
            raise RuntimeError(f"{name}: unidentified or changed verification inputs")
        if report["identity_errors"]:
            raise RuntimeError(f"{name}: identity errors")
        if selection is not None and set(report["selection"]) != selection:
            raise RuntimeError(f"{name}: the observed scope differs from the CI request")
        if report["evaluation"]["complete"]:
            raise RuntimeError(f"{name}: a partial automated run cannot include the real-device review")


    run_phase("check", ["soiree", "check", str(game)], check=True)
    require_fresh_selected_report("check.json")
    published = run_phase("published-negative-control", [
        "soiree", "check", "--fast", "--locked", "--published", str(game)
    ], capture_output=True, text=True, check=False)
    if published.returncode == 0 or "chemin local hors de ce dépôt" not in published.stdout + published.stderr:
        raise RuntimeError(f"the distribution gate did not reject local shared dependencies:\n{published.stdout}\n{published.stderr}")
    run_phase("certify-ui-panic", [
        "soiree", "certify", str(game), "--", "--only", "ui,panic", "--seed", "20261027"
    ], check=True)
    require_fresh_selected_report("verification.json", {
        "harness:health", "harness:ui:U1", "harness:panic:C10"
    })
finally:
    # Preserve only compilation outputs, even when a check fails. Keeping the
    # restored cache outside the game lets the next run create a fresh project.
    target = game / "target"
    if target.exists():
        shutil.move(str(target), str(cached_target))
