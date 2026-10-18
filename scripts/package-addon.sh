#!/usr/bin/env bash
# Build the release archive GdPlanningAI-X.Y.Z.zip from a checkout whose LFS binaries are
# materialized (git lfs pull). GitHub's own source archives hold LFS pointers in place of
# the binaries; this one holds the real files, so it is the release's deliverable.
# It is `git archive` of HEAD (only the addon and script templates, per .gitattributes)
# with the binaries swapped in, then checked: all three present and none an LFS pointer.
# Usage: scripts/package-addon.sh X.Y.Z [OUT_DIR]   (default OUT_DIR: .)
set -euo pipefail

ver=${1:?usage: scripts/package-addon.sh X.Y.Z [OUT_DIR]}
out=${2:-.}
cd "$(dirname "$0")/.."
bin=addons/GdPlanningAI/bin
binaries="$bin/linux/libgdplanningai_rust.so $bin/macos/libgdplanningai_rust.dylib $bin/windows/gdplanningai_rust.dll"

mkdir -p "$out"
out=$(cd "$out" && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "${tmp:?}"' EXIT
name="GdPlanningAI-$ver"

git archive --format=tar --prefix="$name/" HEAD | tar -x -C "$tmp"
for f in $binaries; do
	if head -c 64 "$f" | grep -q '^version https://git-lfs'; then
		echo "$f is an LFS pointer: run git lfs pull first" >&2
		exit 1
	fi
	cp "$f" "$tmp/$name/$f"
done
for f in $binaries; do
	[ "$(wc -c <"$tmp/$name/$f")" -gt 100000 ] || { echo "$f is suspiciously small" >&2; exit 1; }
done

(cd "$tmp" && python3 -c 'import sys, zipfile
src, dest = sys.argv[1:]
import os
with zipfile.ZipFile(dest, "w", zipfile.ZIP_DEFLATED) as z:
    for root, _, files in os.walk(src):
        for f in files:
            p = os.path.join(root, f)
            z.write(p, p)
' "$name" "$out/$name.zip")
echo "$out/$name.zip"
