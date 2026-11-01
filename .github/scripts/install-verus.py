"""Install the exact official Verus archive used by the policy CI job.

The archive hash is checked on cache hits as well as fresh downloads. This
installer belongs to CI; tools/verify-policy.sh never downloads its verifier.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


RELEASE = "0.2026.10.26.d17fbd0"
# Official GitHub release asset digests, read from the release API.
ARCHIVES = {
    ("Linux", "x86_64"): ("x86-linux", "361674d05f0c1b2c9c3f4d5bc56a8fb2ab9241ed0cceca27780668c292bb4940"),
    ("Linux", "aarch64"): ("arm64-linux", "55d0179000c2c78732825d47a8b8fed1aa4afbe6d24896acc6e060eddcd338e4"),
    ("Darwin", "x86_64"): ("x86-macos", "24a23b0267f7a225c6ece4bc019930ca273bd57207fe0c1dd3cc7dc5726a37bc"),
    ("Darwin", "arm64"): ("arm64-macos", "5e62be446a5da782d64caeee41bb1490c8aa3c65c0afe380d1c1dcbe207571ff"),
}


def selected_archive(system, machine):
    try:
        platform, checksum = ARCHIVES[(system, machine)]
    except KeyError as error:
        raise ValueError(f"Unsupported Verus release platform: {system}/{machine}") from error
    filename = f"verus-{RELEASE}-{platform}.zip"
    return {
        "platform": platform,
        "filename": filename,
        "sha256": checksum,
        "url": f"https://github.com/verus-lang/verus/releases/download/release/{RELEASE}/{filename}",
    }


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def install(cache, archive):
    directory = cache / RELEASE / archive["platform"]
    directory.mkdir(parents=True, exist_ok=True)
    download = directory / archive["filename"]
    if not download.exists():
        with tempfile.TemporaryDirectory(prefix="download-", dir=directory) as temporary:
            pending = Path(temporary) / archive["filename"]
            subprocess.run([
                "curl", "--fail", "--silent", "--show-error", "--location",
                "--retry", "3", "--connect-timeout", "20", "--max-time", "300",
                archive["url"], "--output", str(pending),
            ], check=True)
            if sha256(pending) != archive["sha256"]:
                raise ValueError("Official Verus archive checksum mismatch")
            pending.replace(download)
    if sha256(download) != archive["sha256"]:
        raise ValueError(f"Cached Verus archive checksum mismatch: {download}")
    # Re-extract the authenticated archive; cached executables are not trusted
    # merely because an archive beside them has the expected hash.
    installation = directory / "installation"
    with tempfile.TemporaryDirectory(prefix="extract-", dir=directory) as temporary:
        extracted = Path(temporary) / "contents"
        subprocess.run(["unzip", "-q", str(download), "-d", str(extracted)], check=True)
        relative = Path(f"verus-{archive['platform']}") / "verus"
        executable = extracted / relative
        if not executable.is_file() or not os.access(executable, os.X_OK):
            raise ValueError(f"Official archive has no executable at {relative}")
        if installation.exists():
            shutil.rmtree(installation)
        extracted.replace(installation)
    return installation / relative


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("cache", type=Path)
    parser.add_argument("--describe", action="store_true", help="Print the selected asset without installing it")
    args = parser.parse_args()
    host = os.uname()
    archive = selected_archive(host.sysname, host.machine)
    if args.describe:
        print(json.dumps(archive, indent=2))
    else:
        print(install(args.cache.resolve(), archive))


if __name__ == "__main__":
    main()
