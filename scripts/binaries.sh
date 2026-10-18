#!/usr/bin/env bash
# The committed Rust binaries (addons/GdPlanningAI/bin/{linux,macos,windows}, LFS) are
# built by CI. Their identity is a stamp, addons/GdPlanningAI/bin/.source: the git tree id
# of addons/GdPlanningAI/rust, which covers the sources, Cargo.lock, rust-toolchain.toml
# and the Makefile that sets RUSTFLAGS. Binary bytes are never compared (runner image
# updates change them), and the check never pulls LFS objects.
#
#   scripts/binaries.sh key           print the tree id of HEAD's rust/ (or REV's)
#   scripts/binaries.sh check [REV]   (release gate) fail unless the stamp equals the key and all three
#                                     binaries are tracked as LFS pointers
#   scripts/binaries.sh fetch [RUN]   download the build artifacts of a CI run (default: the
#                                     latest build run on HEAD), write the stamp, stage all
#   scripts/binaries.sh stamp         write the stamp for HEAD's rust/ (what fetch does)
#
# PR checks do not gate on this; release.sh and release.yml do. Before a release, refresh:
# on a branch from main, push, wait for the build jobs, run `fetch`, commit the staged
# binaries and stamp, merge. Rust must be committed first: the key is HEAD's tree.
set -euo pipefail

cd "$(dirname "$0")/.."
rust=addons/GdPlanningAI/rust
bin=addons/GdPlanningAI/bin
stamp=$bin/.source
oses="linux/libgdplanningai_rust.so macos/libgdplanningai_rust.dylib windows/gdplanningai_rust.dll"

key() { git rev-parse "${1:-HEAD}:$rust"; }

check() {
	rev=${1:-HEAD}
	want=$(key "$rev")
	have=$(git show "$rev:$stamp" 2>/dev/null | tr -d '[:space:]' || true)
	fail=0
	if [ "$have" != "$want" ]; then
		echo "binaries stamp is stale: $stamp has '${have:-nothing}', $rust is $want" >&2
		echo "rebuild: push, wait for the build jobs, then run scripts/binaries.sh fetch, commit and push" >&2
		fail=1
	fi
	for f in $oses; do
		if ! git show "$rev:$bin/$f" 2>/dev/null | head -c 64 | grep -q '^version https://git-lfs'; then
			echo "missing or not an LFS pointer: $bin/$f" >&2
			fail=1
		fi
	done
	[ "$fail" = 0 ] && echo "binaries match $rust ($want)"
	return "$fail"
}

stamp() {
	if [ -n "$(git status --porcelain -- "$rust")" ]; then
		echo "$rust has uncommitted changes; commit them first (the key is HEAD's tree)" >&2
		exit 1
	fi
	key >"$stamp"
}

fetch() {
	run=${1:-}
	if [ -z "$run" ]; then
		run=$(gh run list --workflow build.yml --commit "$(git rev-parse HEAD)" --status completed \
			--limit 1 --json databaseId --jq '.[0].databaseId')
		[ -n "$run" ] || { echo "no completed build run on HEAD; push and wait for it" >&2; exit 1; }
	fi
	stamp
	tmp=$(mktemp -d)
	trap 'rm -rf "$tmp"' EXIT
	for os in linux macos windows; do
		gh run download "$run" --name "binaries-$os" --dir "$tmp/$os"
		mkdir -p "$bin/$os"
		cp "$tmp/$os"/* "$bin/$os/"
	done
	git add "$bin/linux" "$bin/macos" "$bin/windows" "$stamp"
	echo "staged binaries from run $run, stamp $(cat "$stamp")"
}

case "${1:-}" in
key) key "${2:-}" ;;
check) check "${2:-}" ;;
stamp) stamp ;;
fetch) fetch "${2:-}" ;;
*) sed -n '2,/^set -e/p' "$0" | sed '$d;s/^# \{0,1\}//' ; exit 2 ;;
esac
