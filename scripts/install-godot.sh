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
(cd "$tmp" && grep " $asset\$" SHA512-SUMS.txt | sed 's/  */  /' | shasum -a 512 -c - >&2)

uv run --no-project python -m zipfile -e "$tmp/$asset" "$dir/pkg"
chmod +x "$dir/pkg/$exe"
case "$(uname -s)" in
Darwin) ln -sf "$dir/pkg/$exe" "$dir/godot" ;;
Linux) ln -sf "$dir/pkg/$exe" "$dir/godot" ;;
*) cp "$dir/pkg/$exe" "$dir/godot.exe" ;;
esac
echo "$dir"
