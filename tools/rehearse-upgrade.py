#!/usr/bin/env python3
"""Rehearse what a friend's game does after the tag, before the tag exists.

Every game stays on the published tag until the next one is out, so
`soiree upgrade` refuses and `soiree check` stops at the version line. This
script runs those real commands anyway, from the `soiree` on PATH, against a
fake upstream: a temporary bare clone of this repository's HEAD carrying an
annotated `vX.Y.Z`, reached through a temporary git configuration. Each game is
a clone of its HEAD outside the host's folder, so no `[patch]` applies.

Nothing here makes a game green. An upgrade whose own check fails is reported
and the run continues, because `CHANGELOG.md` allows it; the verdict is
`soiree check --locked --published`, what a game's CI runs.

Before the tag only: once GitHub has it, Cargo's GitHub fast path asks for the
published commit, which this fake upstream does not serve unless it is the same.
"""
import argparse
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
import time

UPSTREAM = "https://github.com/build-your-own-game/soiree"
root = Path(__file__).resolve().parents[1]
host = root.parent


def die(message):
    print(f"refus : {message}", file=sys.stderr)
    sys.exit(2)


def capture(command, cwd=None, env=None):
    completed = subprocess.run(
        command, cwd=cwd, env=env, capture_output=True, text=True, check=False
    )
    if completed.returncode != 0:
        die(f"{shlex.join(map(str, command))} : {completed.stdout}{completed.stderr}")
    return completed.stdout.strip()


def read_only_git(repository, *arguments):
    # Without this, `git status` may refresh the index of a repository that
    # another agent is working in.
    return capture(["git", "--no-optional-locks", "-C", repository, *arguments])


def package_version():
    text = (root / "outil" / "Cargo.toml").read_text().split("[package]", 1)[1]
    found = re.search(r'(?m)^version = "([^"]+)"$', text.split("\n[", 1)[0])
    if found is None:
        die("outil/Cargo.toml : pas de version de paquet")
    return found.group(1)


def require_the_binary(version):
    binary = shutil.which("soiree")
    if binary is None:
        die("pas de soiree dans le PATH : cargo install --locked --path outil")
    found = capture([binary, "version"])
    if found != f"soiree {version}":
        die(f"{binary} dit « {found} », outil/Cargo.toml dit {version} : cargo install --locked --path outil")
    return binary


def require_no_patch_above(path):
    """Cargo reads `.cargo/config.toml` in every parent of a project."""
    for parent in [path, *path.parents]:
        for name in ("config.toml", "config"):
            if (parent / ".cargo" / name).exists():
                die(f"{parent / '.cargo' / name} s'appliquerait aux clones : choisis un --work ailleurs")


def fake_upstream(work, tag, commit):
    """Everything the real upstream has, plus the tag it does not have yet."""
    bare = work / "upstream" / "soiree.git"
    bare.parent.mkdir()
    configuration = work / "gitconfig"
    subprocess.run(["git", "config", "--file", configuration, "user.name", "soiree rehearsal"], check=True)
    subprocess.run(["git", "config", "--file", configuration, "user.email", "rehearsal@soiree.invalid"], check=True)
    subprocess.run(["git", "config", "--file", configuration, f"url.{bare.as_uri()}.insteadOf", UPSTREAM], check=True)
    cargo_home = work / "cargo-home"
    cargo_home.mkdir()
    # Inherited git and Cargo variables could point anywhere, and a pinned
    # toolchain would hide the one each game asks for.
    env = {
        key: value
        for key, value in os.environ.items()
        if not key.startswith(("GIT_", "CARGO_")) and key != "RUSTUP_TOOLCHAIN"
    }
    env |= {
        "GIT_CONFIG_GLOBAL": str(configuration),
        "GIT_CONFIG_NOSYSTEM": "1",
        "CARGO_HOME": str(cargo_home),
        "CARGO_NET_GIT_FETCH_WITH_CLI": "true",
    }
    capture(["git", "clone", "--quiet", "--bare", "--no-hardlinks", root, bare], env=env)
    capture(["git", "-C", bare, "tag", "--force", "--annotate", "-m", f"Rehearsal of {tag}", tag, commit], env=env)
    return env


def changes(clone, env):
    return capture(["git", "status", "--porcelain"], cwd=clone, env=env)


def run_step(name, command, clone, env, logs):
    log = logs / f"{clone.name}-{name}.log"
    print(f"  $ {shlex.join(command)}", flush=True)
    started = time.monotonic()
    with log.open("w") as output:
        code = subprocess.run(
            command, cwd=clone, env=env, stdout=output, stderr=subprocess.STDOUT, check=False
        ).returncode
    print(f"    sortie {code} en {time.monotonic() - started:.0f} s", flush=True)
    return code, log


def errors_of(log):
    """The errors `soiree` marks, or the end of the log if it marks none."""
    lines = log.read_text(errors="replace").splitlines()
    marked = [line.strip() for line in lines if line.strip().startswith("✗")]
    return marked or [line.strip() for line in lines[-12:] if line.strip()]


class Failed(Exception):
    def __init__(self, command, lines):
        super().__init__(command)
        self.command = command
        self.lines = lines


def require_upgraded(clone, version, tag, commit):
    """`soiree upgrade` applied itself, and the fake upstream is what it took."""
    command = "soiree upgrade"
    wanted = re.compile(rf'(?m)^version = "{re.escape(version)}"$')
    if wanted.search((clone / "soiree.toml").read_text()) is None:
        raise Failed(command, [f"soiree.toml n'est pas passé à {version}"])
    lockfile = clone / "Cargo.lock"
    sources = set(re.findall(rf'(?m)^source = "(git\+{re.escape(UPSTREAM)}[^"]*)"$', lockfile.read_text()))
    expected = f"git+{UPSTREAM}?tag={tag}#{commit}"
    if sources != {expected}:
        raise Failed(command, [f"Cargo.lock : attendu {expected}", f"trouvé : {sorted(sources) or 'aucune source du commun'}"])


def rehearse(game, work, logs, env, version, tag, commit):
    head = read_only_git(game, "rev-parse", "HEAD")
    if read_only_git(game, "status", "--porcelain"):
        print(f"  ! {game.name} a des changements non commités : le clone prend {head[:7]}, sans eux")
    clone = work / "games" / game.name
    capture(["git", "clone", "--quiet", "--no-hardlinks", game, clone], env=env)
    capture(["git", "checkout", "--quiet", "--detach", head], cwd=clone, env=env)

    command = ["soiree", "upgrade", "--dry-run"]
    code, log = run_step("upgrade-dry-run", command, clone, env, logs)
    if code != 0:
        raise Failed(shlex.join(command), errors_of(log))
    written = changes(clone, env)
    if written:
        raise Failed(shlex.join(command), [f"a écrit dans le jeu : {written}"])

    command = ["soiree", "upgrade"]
    code, log = run_step("upgrade", command, clone, env, logs)
    if not changes(clone, env):
        raise Failed(shlex.join(command), errors_of(log))
    require_upgraded(clone, version, tag, commit)
    notes = []
    if code != 0:
        # `CHANGELOG.md` says this check may fail on what its later steps fix.
        # The final check below decides.
        notes = [f"soiree upgrade sort {code} après avoir écrit ; journal : {log}", *errors_of(log)]
        for note in notes:
            print(f"  ! {note}")

    command = ["soiree", "check", "--locked", "--published"]
    code, log = run_step("check-published", command, clone, env, logs)
    if code != 0:
        raise Failed(shlex.join(command), errors_of(log))
    return notes


def main():
    parser = argparse.ArgumentParser(description="Répète soiree upgrade sur chaque jeu, contre un faux amont.")
    parser.add_argument("names", nargs="*", metavar="jeu", help="les jeux à répéter (tous ceux de games/ par défaut)")
    parser.add_argument("--work", type=Path, help="où créer le dossier temporaire (défaut : celui du système)")
    parser.add_argument("--keep", action="store_true", help="garde le dossier temporaire et ses journaux")
    parser.add_argument("--keep-going", action="store_true", help="répète les jeux suivants après un échec")
    arguments = parser.parse_args()

    version = package_version()
    tag = f"v{version}"
    binary = require_the_binary(version)
    games = sorted(path.parent for path in (host / "games").glob("*/soiree.toml"))
    if arguments.names:
        games = [game for game in games if game.name in arguments.names]
        missing = set(arguments.names) - {game.name for game in games}
        if missing:
            die(f"pas dans games/ : {', '.join(sorted(missing))}")
    if not games:
        die(f"aucun jeu sous {host / 'games'}")

    base = (arguments.work or Path(tempfile.gettempdir())).resolve()
    base.mkdir(parents=True, exist_ok=True)
    require_no_patch_above(base)
    work = Path(tempfile.mkdtemp(prefix="soiree-rehearsal-", dir=base))
    results = {}
    try:
        commit = read_only_git(root, "rev-parse", "HEAD")
        print(f"Répétition de {tag} : {binary}, faux amont à {commit[:7]}, dans {work}")
        if read_only_git(root, "status", "--porcelain"):
            print("  ! soiree a des changements non commités : le faux amont prend HEAD, sans eux")
        env = fake_upstream(work, tag, commit)
        (work / "games").mkdir()
        logs = work / "logs"
        logs.mkdir()
        for game in games:
            print(f"\n== {game.name}")
            try:
                notes = rehearse(game, work, logs, env, version, tag, commit)
            except Failed as failure:
                results[game.name] = ["FAIL", failure.command, *failure.lines]
                print(f"FAIL {game.name} : {failure.command}")
                for line in failure.lines:
                    print(f"     {line}")
                if not arguments.keep_going:
                    break
            else:
                results[game.name] = ["PASS", *notes]
                print(f"PASS {game.name}")
    finally:
        if arguments.keep:
            print(f"\nGardé : {work}")
        else:
            shutil.rmtree(work)

    print("\nVerdict")
    for game in games:
        verdict = results.get(game.name, ["pas répété"])
        detail = f" : {verdict[1]}" if verdict[0] == "FAIL" else ""
        print(f"  {verdict[0]} {game.name}{detail}")
    return 1 if len(results) < len(games) or any(v[0] == "FAIL" for v in results.values()) else 0


if __name__ == "__main__":
    sys.exit(main())
