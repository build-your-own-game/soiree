"""Exercise the unpublished common checkout through its actual generated game."""
import os
from pathlib import Path
import re
import subprocess

root = Path(__file__).resolve().parents[2]
parent = Path(os.environ["RUNNER_TEMP"]) / "soiree-model"
parent.mkdir()
subprocess.run(["soiree", "new", "ci-model", "--dans", str(parent)], check=True)
game = parent / "ci-model"
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
subprocess.run(["git", "init", "-b", "main"], cwd=game, check=True)
subprocess.run(["soiree", "check", str(game)], check=True)
subprocess.run(["soiree", "certify", str(game), "--", "--only", "ui,panic", "--seed", "20261027"], check=True)
