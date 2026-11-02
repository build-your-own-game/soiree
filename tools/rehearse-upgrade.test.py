"""The rehearsal's contract, against a fake `soiree` in a fake host folder.

Real git, a fake binary: this proves what the script refuses, what reaches the
commands it runs, and that it can only say PASS from the final check. The real
`soiree upgrade` and Cargo are what the script itself exercises.
"""
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile
import textwrap
import unittest

SCRIPT = Path(__file__).resolve().with_name("rehearse-upgrade.py")
UPSTREAM = "https://github.com/build-your-own-game/soiree"
VERSION = "9.9.9"

# Stands in for `soiree`: says what it saw, then applies what the case asks for
# through `FAKE`, as {game: {command: exit code, ...}}.
FAKE_SOIREE = textwrap.dedent(f'''\
    #!{sys.executable}
    import json, os, pathlib, re, subprocess, sys
    arguments = sys.argv[1:]
    if arguments == ["version"]:
        print("soiree " + os.environ.get("FAKE_VERSION", "{VERSION}"))
        sys.exit(0)
    game = pathlib.Path.cwd()
    case = json.loads(os.environ.get("FAKE", "{{}}")).get(game.name, {{}})
    with open(os.environ["FAKE_LOG"], "a") as log:
        log.write(json.dumps({{"arguments": arguments, "cwd": str(game), "env": dict(os.environ)}}) + "\\n")
    if arguments == ["upgrade", "--dry-run"] and case.get("dry_run_writes"):
        (game / "written").write_text("")
    if arguments == ["upgrade"]:
        if case.get("refuse"):
            print("  ✗ refusé avant toute écriture")
            sys.exit(1)
        listed = subprocess.run(
            ["git", "ls-remote", "--exit-code", "--tags", "{UPSTREAM}", "refs/tags/v{VERSION}^{{}}"],
            capture_output=True, text=True, check=True,
        ).stdout.split()[0]
        (game / "soiree.toml").write_text('version = "{VERSION}"\\n')
        sha = case.get("locked_commit", listed)
        (game / "Cargo.lock").write_text(
            '[[package]]\\nname = "salon-core"\\nsource = "git+{UPSTREAM}?tag=v{VERSION}#' + sha + '"\\n'
        )
        sys.exit(case.get("upgrade", 0))
    if arguments[0] == "check":
        print("  ✗ le contrôle final refuse" if case.get("check") else "  ✓ rien")
        sys.exit(case.get("check", 0))
''')


class Rehearsal(unittest.TestCase):
    def setUp(self):
        base = Path(tempfile.mkdtemp(prefix="rehearsal-test-")).resolve()
        self.addCleanup(shutil.rmtree, base)
        self.scratch = base / "scratch"
        self.scratch.mkdir()
        self.host = base / "host"
        (self.host / ".cargo").mkdir(parents=True)
        (self.host / ".cargo" / "config.toml").write_text("[patch]\n")
        self.common = self.host / "soiree"
        (self.common / "tools").mkdir(parents=True)
        (self.common / "outil").mkdir()
        shutil.copy(SCRIPT, self.common / "tools")
        (self.common / "outil" / "Cargo.toml").write_text(f'[package]\nname = "soiree"\nversion = "{VERSION}"\n\n[lib]\nversion = "0"\n')
        self.repository(self.common)
        self.games = {name: self.game(name) for name in ("alpha", "beta")}
        self.binaries = base / "bin"
        self.binaries.mkdir()
        fake = self.binaries / "soiree"
        fake.write_text(FAKE_SOIREE)
        fake.chmod(fake.stat().st_mode | stat.S_IEXEC)
        self.log = base / "seen.jsonl"
        self.log.touch()

    def repository(self, path):
        for command in (
            ["init", "-q", "-b", "main"],
            ["add", "."],
            ["-c", "user.name=t", "-c", "user.email=t@t.invalid", "commit", "-qm", "start"],
        ):
            subprocess.run(["git", "-C", path, *command], check=True)

    def game(self, name):
        game = self.host / "games" / name
        game.mkdir(parents=True)
        (game / "soiree.toml").write_text('version = "0.0.1"\n')
        self.repository(game)
        return game

    def run_script(self, *arguments, fake=None, version=VERSION):
        env = {key: value for key, value in os.environ.items() if not key.startswith("GIT_")}
        env |= {
            "PATH": f"{self.binaries}{os.pathsep}{os.environ['PATH']}",
            "FAKE": json.dumps(fake or {}),
            "FAKE_LOG": str(self.log),
            "FAKE_VERSION": version,
            "TMPDIR": str(self.scratch),
        }
        return subprocess.run(
            [sys.executable, self.common / "tools" / SCRIPT.name, *arguments],
            env=env, capture_output=True, text=True, check=False,
        )

    def seen(self):
        return [json.loads(line) for line in self.log.read_text().splitlines()]

    def test_every_game_passes_outside_the_host_on_its_own_temporary_state(self):
        before = {name: list(game.glob("*")) for name, game in self.games.items()}
        run = self.run_script()
        self.assertEqual(run.returncode, 0, run.stdout + run.stderr)
        self.assertIn("PASS alpha", run.stdout)
        self.assertIn("PASS beta", run.stdout)
        commands = [record["arguments"] for record in self.seen()]
        self.assertEqual(commands, 2 * [["upgrade", "--dry-run"], ["upgrade"], ["check", "--locked", "--published"]])
        for record in self.seen():
            self.assertFalse(Path(record["cwd"]).is_relative_to(self.host), record["cwd"])
            self.assertTrue(Path(record["env"]["CARGO_HOME"]).is_relative_to(self.scratch))
            self.assertTrue(Path(record["env"]["GIT_CONFIG_GLOBAL"]).is_relative_to(self.scratch))
            self.assertEqual(record["env"]["CARGO_NET_GIT_FETCH_WITH_CLI"], "true")
        self.assertEqual({name: list(game.glob("*")) for name, game in self.games.items()}, before)
        self.assertEqual(list(self.scratch.iterdir()), [], "the temporary folder must go away")
        tags = subprocess.run(["git", "-C", self.common, "tag"], capture_output=True, text=True, check=True)
        self.assertEqual(tags.stdout, "", "the real repository never gets the tag")

    def test_another_binary_is_refused_before_any_clone(self):
        run = self.run_script(version="0.0.1")
        self.assertEqual(run.returncode, 2)
        self.assertIn(VERSION, run.stderr)
        self.assertEqual(self.seen(), [])
        self.assertEqual(list(self.scratch.iterdir()), [])

    def test_a_work_folder_under_the_patch_is_refused(self):
        run = self.run_script("--work", str(self.host / "games"))
        self.assertEqual(run.returncode, 2)
        self.assertIn("config.toml", run.stderr)
        self.assertEqual(self.seen(), [])

    def test_the_first_final_failure_stops_and_says_what_failed(self):
        run = self.run_script(fake={"alpha": {"check": 1}})
        self.assertEqual(run.returncode, 1)
        self.assertIn("FAIL alpha : soiree check --locked --published", run.stdout)
        self.assertIn("le contrôle final refuse", run.stdout)
        self.assertIn("pas répété beta", run.stdout)
        self.assertNotIn("beta", {Path(record["cwd"]).name for record in self.seen()})

    def test_keep_going_gives_every_verdict_and_still_fails(self):
        run = self.run_script("--keep-going", fake={"alpha": {"check": 1}})
        self.assertEqual(run.returncode, 1)
        self.assertIn("FAIL alpha", run.stdout)
        self.assertIn("PASS beta", run.stdout)

    def test_an_upgrade_whose_own_check_fails_is_only_final_through_the_last_check(self):
        run = self.run_script(fake={"alpha": {"upgrade": 1}})
        self.assertEqual(run.returncode, 0, run.stdout)
        self.assertIn("soiree upgrade sort 1", run.stdout)
        self.assertIn("PASS alpha", run.stdout)

    def test_a_refused_upgrade_is_final(self):
        run = self.run_script(fake={"alpha": {"refuse": True}})
        self.assertEqual(run.returncode, 1)
        self.assertIn("FAIL alpha : soiree upgrade", run.stdout)
        self.assertIn("refusé avant toute écriture", run.stdout)

    def test_a_dry_run_that_writes_fails(self):
        run = self.run_script(fake={"alpha": {"dry_run_writes": True}})
        self.assertEqual(run.returncode, 1)
        self.assertIn("FAIL alpha : soiree upgrade --dry-run", run.stdout)

    def test_a_lock_that_did_not_take_the_fake_upstream_fails(self):
        run = self.run_script(fake={"alpha": {"locked_commit": "0" * 40}})
        self.assertEqual(run.returncode, 1)
        self.assertIn("FAIL alpha : soiree upgrade", run.stdout)
        self.assertIn("Cargo.lock : attendu", run.stdout)


if __name__ == "__main__":
    unittest.main()
