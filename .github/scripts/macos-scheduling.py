"""Observe the native room's scheduling; this is not a certification or fix.

All samples use one freshly identified libtest executable and unchanged room
code. Controlled policy changes apply only to the explicitly named children.
No result is judged against or substituted for a game's performance budget.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import resource
import signal
import subprocess
import time


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--suite", choices=("qos", "timer-precision"), default="timer-precision")
    args = parser.parse_args()
    if platform.system() != "Darwin":
        parser.error("this diagnostic requires macOS")
    root = Path(__file__).resolve().parents[2]
    output = args.output_dir.resolve()
    output.mkdir(parents=True, exist_ok=False)
    report = {
        "schema": 1,
        "diagnostic_only": True,
        "suite": args.suite,
        "platform": platform.platform(),
        "samples": [],
        "limitations": [
            "One fixed three-second sample per case; no statistical certification.",
            "Injected policy reader observes the process and main thread at startup, not the room thread's QoS.",
            "Requested QoS and readable base policies do not reveal every effective scheduler constraint.",
            "An instrumented sample can differ from an uninstrumented sample; both baselines are retained.",
            "Controlled utility/background children demonstrate a mechanism, not the CI job's original policy.",
            "A successful main-thread QoS request does not prove the QoS inherited by the room thread or override a task clamp.",
            "The optional pthread introspection hook records requested QoS at thread START, before the Rust callback; this is not effective QoS or a promise it stays unchanged.",
            "A Foundation activity token or a successful taskpolicy request is a hint, not proof of effective timer precision.",
        ],
    }

    def save():
        (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")

    def run(name, command, cwd=root, timeout=30, isolated_build=False):
        started = time.monotonic()
        before = resource.getrusage(resource.RUSAGE_CHILDREN)
        # Keep the controller in its inherited group. The generated Rust
        # launcher itself applies the production child's process_group(0).
        child = subprocess.Popen(command, cwd=cwd, stdin=subprocess.DEVNULL,
                                 stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                 start_new_session=isolated_build)
        try:
            stdout, stderr = child.communicate(timeout=timeout)
        except subprocess.TimeoutExpired:
            if isolated_build:
                os.killpg(child.pid, signal.SIGKILL)
            elif command[0] == str(output / "production-launcher"):
                # The production helper normally enforces its own earlier
                # deadline. Clean its one isolated test child if it broke.
                children = subprocess.run(["/usr/bin/pgrep", "-P", str(child.pid)],
                                          capture_output=True, text=True, check=False)
                for value in children.stdout.split():
                    pid = int(value)
                    try:
                        if os.getpgid(pid) == pid:
                            os.killpg(pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                child.kill()
            else:
                child.kill()
            stdout, stderr = child.communicate()
            timed_out = True
        else:
            timed_out = False
        after = resource.getrusage(resource.RUSAGE_CHILDREN)
        (output / f"{name}.stdout").write_bytes(stdout)
        (output / f"{name}.stderr").write_bytes(stderr)
        sample = {
            "name": name, "command": command, "cwd": str(cwd),
            "exit_code": child.returncode,
            "controller_timeout": timed_out,
            "isolated_build_session": isolated_build,
            "elapsed_seconds": time.monotonic() - started,
            "child_cpu_seconds": after.ru_utime + after.ru_stime - before.ru_utime - before.ru_stime,
            "stdout": f"{name}.stdout", "stderr": f"{name}.stderr",
            "policy": [json.loads(line.removeprefix("SOIREE_MACOS_POLICY "))
                       for line in stderr.decode(errors="replace").splitlines()
                       if line.startswith("SOIREE_MACOS_POLICY ")],
            "qos_requests": [json.loads(line.removeprefix("SOIREE_MACOS_QOS_REQUEST "))
                             for line in stderr.decode(errors="replace").splitlines()
                             if line.startswith("SOIREE_MACOS_QOS_REQUEST ")],
            "thread_creations": [json.loads(line.removeprefix("SOIREE_MACOS_THREAD_CREATE "))
                                 for line in stderr.decode(errors="replace").splitlines()
                                 if line.startswith("SOIREE_MACOS_THREAD_CREATE ")],
            "activities": [json.loads(line.removeprefix("SOIREE_MACOS_ACTIVITY "))
                           for line in stderr.decode(errors="replace").splitlines()
                           if line.startswith("SOIREE_MACOS_ACTIVITY ")],
        }
        report["samples"].append(sample)
        save()
        print(f"{name}: exit {child.returncode}, {sample['elapsed_seconds']:.3f}s", flush=True)
        if b"wakes," in stdout or name.endswith("-version"):
            print(stdout.decode(errors="replace"), flush=True)
        if timed_out or child.returncode:
            print(stderr.decode(errors="replace")[-5000:], flush=True)
            raise RuntimeError(f"{name}: command failed; see retained output")
        return stdout, sample

    try:
        report["commit"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
        report["git_status"] = subprocess.check_output(["git", "status", "--porcelain"], cwd=root, text=True)
        report["launch_environment"] = {key: os.environ[key] for key in (
            "CI", "GITHUB_RUN_ID", "GITHUB_RUN_ATTEMPT", "RUNNER_OS", "RUNNER_ARCH",
            "RUSTUP_TOOLCHAIN", "CARGO_TARGET_DIR", "RUST_TEST_THREADS") if key in os.environ}
        sources = [Path(__file__).resolve(), root / ".github/scripts/macos-policy.c",
                   root / ".github/scripts/macos-qos-request.c",
                   root / ".github/scripts/macos-activity.m",
                   root / "salon/crates/host/src/room.rs", root / "salon/Cargo.lock",
                   root / "outil/src/verification/cargo.rs",
                   root / ".github/workflows/macos-scheduling.yml"]
        report["source_sha256"] = {str(path.relative_to(root)): sha256(path) for path in sources}
        run("rustc-version", ["rustup", "run", "1.99", "rustc", "-vV"])
        run("clang-version", ["xcrun", "clang", "--version"])
        run("macos-version", ["/usr/bin/sw_vers"])
        build, _ = run("build", ["cargo", "+1.99", "test", "--locked", "-p", "salon-host", "--lib",
                                  "--no-run", "--message-format=json"], cwd=root / "salon", timeout=900,
                              isolated_build=True)
        artifacts = [json.loads(line) for line in build.decode().splitlines() if line.startswith("{")]
        executables = {value["executable"] for value in artifacts
                       if value.get("reason") == "compiler-artifact" and value.get("executable")
                       and value["target"]["name"] == "salon_host" and value["profile"]["test"]}
        if len(executables) != 1:
            raise RuntimeError("expected exactly one salon-host libtest executable")
        executable = Path(executables.pop())
        report["test_executable"] = str(executable)
        report["test_executable_sha256"] = sha256(executable)

        # Compile the current production launch implementation, not a Python
        # approximation of its groups, pipe reader threads, and polling loop.
        source = (root / "outil/src/verification/cargo.rs").read_text()
        runner = source[source.index("fn execute_with_env("):source.index("fn output_text(")]
        runner_path = output / "production-launcher.rs"
        runner_path.write_text("""use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};
""" + runner + """
fn main() {
    use std::io::Write;
    let command: Vec<String> = std::env::args().skip(1).collect();
    let dir = std::env::current_dir().expect("diagnostic cwd");
    match execute_with_env(&dir, &command, 20, &BTreeMap::new()) {
        Ok(output) => {
            std::io::stdout().write_all(&output.stdout).expect("diagnostic stdout");
            std::io::stderr().write_all(&output.stderr).expect("diagnostic stderr");
            std::process::exit(output.status.code().unwrap_or(1));
        }
        Err(error) => { eprintln!("{error}"); std::process::exit(1); }
    }
}
""")
        launcher = output / "production-launcher"
        run("build-launcher", ["rustup", "run", "1.99", "rustc", "--edition=2024", str(runner_path), "-o", str(launcher)],
            isolated_build=True)
        policy = output / "policy.dylib"
        run("build-policy-reader", ["xcrun", "clang", "-Wall", "-Wextra", "-Werror", "-dynamiclib",
                                      str(root / ".github/scripts/macos-policy.c"), "-o", str(policy)], isolated_build=True)
        report["launcher_sha256"] = sha256(launcher)
        report["extracted_runner_source_sha256"] = sha256(runner_path)
        report["policy_reader_sha256"] = sha256(policy)
        qos_request = output / "controlled-qos-request.dylib"
        run("build-controlled-qos-request", ["xcrun", "clang", "-Wall", "-Wextra", "-Werror", "-dynamiclib",
                                              str(root / ".github/scripts/macos-qos-request.c"), "-o", str(qos_request)],
            isolated_build=True)
        report["controlled_qos_request_sha256"] = sha256(qos_request)
        activity = output / "controlled-activity.dylib"
        run("build-controlled-activity", ["xcrun", "clang", "-Wall", "-Wextra", "-Werror", "-dynamiclib", "-fobjc-arc",
                                          "-framework", "Foundation", str(root / ".github/scripts/macos-activity.m"),
                                          "-o", str(activity)], isolated_build=True)
        report["controlled_activity_sha256"] = sha256(activity)
        base = [str(executable), "room::tests::steps_come_on_time", "--exact", "--ignored",
                "--nocapture", "--test-threads=1"]
        injected = ["/usr/bin/env", f"DYLD_INSERT_LIBRARIES={policy}", *base]
        user_initiated = ["/usr/bin/env", f"DYLD_INSERT_LIBRARIES={qos_request}",
                          "SOIREE_MACOS_CONTROLLED_QOS=user-initiated", *base]
        observed_threads = ["/usr/bin/env", f"DYLD_INSERT_LIBRARIES={qos_request}",
                            "SOIREE_MACOS_CONTROLLED_QOS=observe-threads", *base]
        qos_cases = [
            ("direct", base, 0, 0),
            ("production-launch", [str(launcher), *base], 0, 0),
            ("production-launch-policy", [str(launcher), *injected], 1, 0),
            ("controlled-utility", [str(launcher), "/usr/sbin/taskpolicy", "-c", "utility", *injected], 1, 0),
            ("controlled-background", [str(launcher), "/usr/sbin/taskpolicy", "-b", *injected], 1, 0),
            ("controlled-application", [str(launcher), "/usr/sbin/taskpolicy", "-a", *injected], 1, 0),
            ("observed-threads", [str(launcher), *observed_threads], -1, 0),
            ("controlled-threads-user-initiated", [str(launcher), *user_initiated], -1, -1),
            ("controlled-application-threads-user-initiated", [str(launcher), "/usr/sbin/taskpolicy", "-a", *user_initiated], -1, -1),
        ]
        def activity_case(name, mode, policy_count):
            command = [str(launcher), "/usr/bin/env", f"DYLD_INSERT_LIBRARIES={qos_request}:{activity}",
                       "SOIREE_MACOS_CONTROLLED_QOS=user-initiated",
                       f"SOIREE_MACOS_CONTROLLED_ACTIVITY={mode}", *base]
            return name, command, policy_count, -1

        precision_cases = [
            ("direct", base, 0, 0),
            ("production-launch", [str(launcher), *base], 0, 0),
            ("production-launch-policy", [str(launcher), *injected], 1, 0),
            ("precision-qos-baseline", [str(launcher), *user_initiated], -1, -1),
            activity_case("foundation-without-activity", "observe", -1),
            activity_case("controlled-user-activity-allow-sleep", "user-initiated-allow-sleep", -1),
            activity_case("controlled-latency-critical-activity", "latency-critical", -1),
            ("controlled-latency-tier0", [str(launcher), "/usr/sbin/taskpolicy", "-l", "0", *user_initiated], -1, -1),
        ]
        if args.suite == "timer-precision":
            report["counterfactuals"] = {
                "common_reference": "USER_INITIATED requested on main and workers, as in the previously unsuccessful QoS counter-test",
                "controlled-user-activity-allow-sleep": {
                    "options": ["NSActivityUserInitiatedAllowingIdleSystemSleep"],
                    "source": "https://developer.apple.com/documentation/foundation/processinfo/activityoptions/userinitiatedallowingidlesystemsleep",
                },
                "controlled-latency-critical-activity": {
                    "options": ["NSActivityUserInitiatedAllowingIdleSystemSleep", "NSActivityLatencyCritical"],
                    "source": "https://developer.apple.com/documentation/foundation/processinfo/activityoptions/latencycritical",
                },
                "controlled-latency-tier0": {
                    "tier": "LATENCY_QOS_TIER_0", "tier_raw": 0xFF0001,
                    "source": "https://github.com/apple-oss-distributions/system_cmds/blob/main/taskpolicy/taskpolicy.c",
                    "limitation": "A task latency override does not clear a task QoS clamp or the latency tier derived from a worker's effective QoS.",
                },
            }
        cases = qos_cases if args.suite == "qos" else precision_cases
        for name, command, policy_count, request_count in cases:
            stdout, sample = run(name, command, cwd=root / "salon", timeout=45)
            if ((policy_count >= 0 and len(sample["policy"]) != policy_count)
                    or (request_count >= 0 and len(sample["qos_requests"]) != request_count)):
                raise RuntimeError(f"{name}: expected policy reads/QoS requests were not observed (SIP may strip DYLD variables)")
            if name == "foundation-without-activity":
                if [value["event"] for value in sample["activities"]] != ["observe"]:
                    raise RuntimeError(f"{name}: missing read-only Foundation baseline")
            if name in ("controlled-user-activity-allow-sleep", "controlled-latency-critical-activity"):
                if ([value["event"] for value in sample["activities"]] != ["begin", "end"]
                        or not all(value["token_created"] for value in sample["activities"])):
                    raise RuntimeError(f"{name}: activity token was not retained through normal process exit")
            if policy_count < 0:
                workers = {value["thread_id"] for value in sample["policy"]
                           if value.get("phase") == "thread-start-before-request"}
                requested = {value["thread_id"] for value in sample["qos_requests"]}
                if len(workers) < 2 or len(sample["thread_creations"]) < 2:
                    raise RuntimeError(f"{name}: missing libtest/room thread observations")
                if request_count < 0 and not workers.issubset(requested):
                    raise RuntimeError(f"{name}: a started worker did not receive the controlled QoS request")
            if not re.search(rb"[1-9][0-9]* wakes, gaps", stdout):
                raise RuntimeError(f"{name}: no usable cadence measurement was emitted")
            if b"1 passed; 0 failed; 0 ignored" not in stdout:
                raise RuntimeError(f"{name}: the requested timing probe was not actually executed")
        report["completed"] = True
    except BaseException as error:
        report["error"] = str(error)
        raise
    finally:
        save()


if __name__ == "__main__":
    main()
