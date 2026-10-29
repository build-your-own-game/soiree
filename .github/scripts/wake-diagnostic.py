"""Compare the exact room wait with independent std primitives. Diagnostic only."""
import hashlib
import json
import os
from pathlib import Path
import platform
import resource
import subprocess
import sys
import time

root = Path(__file__).resolve().parents[2]
output = Path(sys.argv[1]).resolve()
output.mkdir(parents=True, exist_ok=False)
suite = sys.argv[2] if len(sys.argv) > 2 else "std"
if suite not in ("std", "native", "coalescing"):
    raise ValueError("unknown diagnostic suite")
report = {"diagnostic_only": True, "not_a_performance_attestation": True,
          "suite": suite,
          "platform": platform.platform(), "machine": platform.machine(),
          "cpu_count": os.cpu_count(), "samples": [],
          "commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip(),
          "git_status": subprocess.check_output(["git", "status", "--porcelain"], cwd=root, text=True),
          "environment": {key: os.environ[key] for key in ("GITHUB_RUN_ID", "ImageVersion", "RUNNER_OS", "RUNNER_ARCH") if key in os.environ},
          "limitations": ["Fixed two rounds, reverse order on round two; never retry until green.",
                          "Three-second samples diagnose a mechanism; they do not certify a game.",
                          "CPU time covers the child process, not only its timed loop.",
                          "Clock-read gaps in spin can show preemption but cannot identify its owner."]}

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def save():
    (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")

def command(name, args):
    result = subprocess.run(args, cwd=root, capture_output=True, timeout=180)
    (output / f"{name}.stdout").write_bytes(result.stdout)
    (output / f"{name}.stderr").write_bytes(result.stderr)
    report[name] = {"command": args, "exit_code": result.returncode}
    result.check_returncode()
    return result

original_coalescing = None
try:
    if suite == "coalescing":
        # This is a global kernel counterfactual, confined to a disposable
        # GitHub-hosted VM. Never run it against the host's personal Mac.
        if (platform.system() != "Darwin" or os.environ.get("GITHUB_ACTIONS") != "true"
                or os.environ.get("RUNNER_ENVIRONMENT") != "github-hosted"):
            raise RuntimeError("coalescing mutation is confined to a disposable GitHub-hosted macOS VM")
        original_coalescing = subprocess.check_output(["sysctl", "-n", "kern.timer.coalescing_enabled"], text=True).strip()
        if original_coalescing != "1":
            original_coalescing = None
            raise RuntimeError("expected coalescing enabled at baseline")
        report["original_coalescing"] = original_coalescing
    room = (root / "salon/crates/host/src/room.rs").read_text()
    wait = room[room.index("const SPIN:"):room.index("pub fn run<G:")]
    template = root / ".github/scripts/wake-probe.rs"
    source = output / "probe.rs"
    source.write_text(template.read_text().replace("// PRODUCTION_WAIT", wait))
    report["source_sha256"] = {str(path.relative_to(root)): digest(path) for path in
                               (template, Path(__file__).resolve(), root / "salon/crates/host/src/room.rs")}
    report["extracted_production_wait_sha256"] = hashlib.sha256(wait.encode()).hexdigest()
    report["generated_source_sha256"] = digest(source)
    command("rustc-version", ["rustup", "run", "1.99", "rustc", "-vV"])
    executable = output / "probe"
    command("build", ["rustup", "run", "1.99", "rustc", "--edition=2024", "-Dwarnings", str(source), "-o", str(executable)])
    report["binary_sha256"] = digest(executable)
    if platform.system() == "Darwin":
        command("host", ["sysctl", "hw.model", "hw.ncpu", "hw.memsize", "kern.osrelease"])
        command("timer-policy", ["sysctl", "kern.timer.coalescing_enabled", "kern.timer_coalesce_bg_ns_max",
                                 "kern.timer_coalesce_tier3_ns_max"])
    else:
        command("host", ["uname", "-a"])
        for item in ("/proc/self/cgroup", "/proc/loadavg", "/sys/fs/cgroup/cpu.max", "/sys/fs/cgroup/cpu.stat"):
            path = Path(item)
            if path.exists():
                report[item] = path.read_text()
    modes = ["room-wait", "recv-timeout", "sleep", "park", "condvar", "yield", "spin"]
    if report["suite"] == "native":
        if platform.system() != "Darwin":
            raise RuntimeError("native timer diagnostic requires macOS")
        native = root / ".github/scripts/native-wake-probe.c"
        report["source_sha256"][str(native.relative_to(root))] = digest(native)
        executable = output / "native-probe"
        command("build-native", ["xcrun", "clang", "-Wall", "-Wextra", "-Werror", "-pthread", str(native), "-o", str(executable)])
        report["native_binary_sha256"] = digest(executable)
        modes = ["nanosleep", "dispatch-semaphore", "mach-wait", "kqueue-normal", "kqueue-critical"]
    rounds = [(None, modes), (None, list(reversed(modes)))]
    if suite == "coalescing":
        modes = ["room-wait", "recv-timeout", "sleep"]
        rounds = [(1, modes), (0, modes), (0, list(reversed(modes))), (1, list(reversed(modes)))]
    for round_index, (coalescing, ordered) in enumerate(rounds):
        if coalescing is not None:
            command(f"round-{round_index}-set-coalescing", ["sudo", "-n", "sysctl", "-w", f"kern.timer.coalescing_enabled={coalescing}"])
            observed = subprocess.check_output(["sysctl", "-n", "kern.timer.coalescing_enabled"], text=True).strip()
            if observed != str(coalescing):
                raise RuntimeError("kernel control did not take effect")
        for mode in ordered:
            before = resource.getrusage(resource.RUSAGE_CHILDREN)
            started = time.monotonic()
            result = command(f"round-{round_index}-{mode}", [str(executable), mode])
            after = resource.getrusage(resource.RUSAGE_CHILDREN)
            sample = json.loads(result.stdout)
            if len(sample["samples_gap_lateness_ns"]) < 2:
                raise RuntimeError("no usable observations")
            gaps = sorted(pair[0] / 1e6 for pair in sample["samples_gap_lateness_ns"])
            late = sorted(pair[1] / 1e6 for pair in sample["samples_gap_lateness_ns"])
            sample.update(round=round_index, wall_seconds=time.monotonic() - started,
                          kernel_coalescing=coalescing,
                          cpu_seconds=after.ru_utime + after.ru_stime - before.ru_utime - before.ru_stime,
                          count=len(gaps), gap_p95_ms=gaps[((len(gaps)*95+99)//100)-1],
                          gap_max_ms=max(gaps), lateness_p95_ms=late[((len(late)*95+99)//100)-1])
            report["samples"].append(sample)
            save()
            print(f"round {round_index} {mode}: n={len(gaps)} p95={sample['gap_p95_ms']:.3f}ms max={max(gaps):.3f}ms CPU={sample['cpu_seconds']:.3f}s", flush=True)
    report["completed"] = True
finally:
    try:
        if original_coalescing is not None:
            command("restore-coalescing", ["sudo", "-n", "sysctl", "-w", f"kern.timer.coalescing_enabled={original_coalescing}"])
            restored = subprocess.check_output(["sysctl", "-n", "kern.timer.coalescing_enabled"], text=True).strip()
            report["restored_coalescing"] = restored
            if restored != original_coalescing:
                raise RuntimeError("kernel control restoration failed")
    finally:
        save()
