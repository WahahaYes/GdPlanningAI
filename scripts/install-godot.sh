#!/usr/bin/env bash
# Install the Godot version pinned in global.json (headless use) into DIR and
# print DIR, which then holds a `godot` executable. Verifies the download
# against the release's SHA512-SUMS.txt. Linux, macOS and Windows (Git Bash).
# Usage: scripts/install-godot.sh [DIR]   (default: $RUNNER_TEMP/godot or ./.godot-bin)
set -euo pipefail

cd "$(dirname "$0")/.."
version=$(make --no-print-directory godot-version)
dir=${1:-${RUNNER_TEMP:-$PWD/.godot-bin}/godot}
mkdir -p "$dir"
dir=$(cd "$dir" && pwd)

release="${version}-stable"
base="https://github.com/godotengine/godot-builds/releases/download/${release}"
case "$(uname -s)" in
Linux) asset="Godot_v${release}_linux.x86_64.zip" exe="Godot_v${release}_linux.x86_64" ;;
Darwin) asset="Godot_v${release}_macos.universal.zip" exe="Godot.app/Contents/MacOS/Godot" ;;
*) asset="Godot_v${release}_win64.exe.zip" exe="Godot_v${release}_win64_console.exe" ;;
esac

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
curl -fsSL --retry 3 -o "$tmp/$asset" "$base/$asset"
curl -fsSL --retry 3 -o "$tmp/SHA512-SUMS.txt" "$base/SHA512-SUMS.txt"
uv run --no-project python - "$tmp/$asset" "$tmp/SHA512-SUMS.txt" <<'PY'
import hashlib, sys

asset, sums = sys.argv[1:]
name = asset.replace("\\", "/").rsplit("/", 1)[-1]
want = next(l.split()[0] for l in open(sums) if l.split()[-1].lstrip("*") == name)
got = hashlib.sha512(open(asset, "rb").read()).hexdigest()
if got != want:
    sys.exit(f"{name}: sha512 mismatch")
print(f"{name}: OK", file=sys.stderr)
PY

uv run --no-project python -m zipfile -e "$tmp/$asset" "$dir/pkg"
chmod +x "$dir/pkg/$exe"
case "$(uname -s)" in
Darwin) ln -sf "$dir/pkg/$exe" "$dir/godot" ;;
Linux) ln -sf "$dir/pkg/$exe" "$dir/godot" ;;
*)
	# The _console exe is a wrapper that launches the plain exe of the same base name,
	# so both are kept; `godot` is a shim for the wrapper (Git Bash finds it before godot.exe).
	cp "$dir/pkg/Godot_v${release}_win64.exe" "$dir/godot.exe"
	cp "$dir/pkg/$exe" "$dir/godot_console.exe"
	printf '#!/bin/sh\nexec "$(dirname "$0")/godot_console.exe" "$@"\n' >"$dir/godot"
	chmod +x "$dir/godot"
	;;
esac
echo "$dir"
