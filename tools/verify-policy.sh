#!/usr/bin/env bash
# Verify the same finite policy source that the normal soiree CLI compiles.
set -euo pipefail
policy_script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
exec python3 "$policy_script_dir/../outil/verification-kernel/verify.py" "$@"
