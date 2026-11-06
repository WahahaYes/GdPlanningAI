#!/usr/bin/env bash
# Smoke-test the addon on the oldest Godot it claims to support (compatibility_minimum in
# gdplanningai.gdextension, kept equal to the godot crate's api-4-N feature by the godot-floor
# necro rule). Lays the addon out as the release zip does (git archive of HEAD per
# .gitattributes, plus the Linux library from addons/GdPlanningAI/bin/linux), adds
# test/compat/smoke.gd as the main scene's script, imports, and runs it headless. Fails on a
# smoke failure, a timeout, or any error in Godot's log. Linux only.
# Usage: scripts/compat-smoke.sh GODOT [WORK_DIR]   (GODOT: that version's editor binary)
set -euo pipefail

godot=${1:?usage: scripts/compat-smoke.sh GODOT [WORK_DIR]}
godot=$(cd "$(dirname "$godot")" && pwd)/$(basename "$godot")
cd "$(dirname "$0")/.."
keep=${2:+1}
work=${2:-$(mktemp -d)}
mkdir -p "$work"
work=$(cd "$work" && pwd)
lib=addons/GdPlanningAI/bin/linux/libgdplanningai_rust.so

if [ ! -s "$lib" ] || head -c 64 "$lib" | grep -q '^version https://git-lfs'; then
	echo "$lib is missing: run make -C addons/GdPlanningAI/rust build-release first" >&2
	exit 1
fi
min=$(make --no-print-directory godot-min-version)

proj=${work:?}/project
rm -rf "$proj"
mkdir -p "$proj"
# stash create snapshots uncommitted changes to tracked files (HEAD when there are none), so a
# local run tests the working tree.
tree=$(git stash create)
git archive --format=tar "${tree:-HEAD}" | tar -x -C "$proj"
mkdir -p "$proj/addons/GdPlanningAI/bin/linux"
cp "$lib" "$proj/addons/GdPlanningAI/bin/linux/"
cp test/compat/smoke.gd "$proj/smoke.gd"
# Hand-written in the oldest formats Godot 4 reads, so no editor version can have drifted them.
cat >"$proj/project.godot" <<EOF
config_version=5

[application]

config/name="GdPlanningAI compat smoke"
run/main_scene="res://main.tscn"
config/features=PackedStringArray("$min")

[autoload]

GdPAIAutoload="*res://addons/GdPlanningAI/gdpai_autoload.gd"

[editor_plugins]

enabled=PackedStringArray("res://addons/GdPlanningAI/plugin.cfg")
EOF
cat >"$proj/main.tscn" <<'EOF'
[gd_scene load_steps=2 format=3]

[ext_resource type="Script" path="res://smoke.gd" id="1"]

[node name="Main" type="Node"]
script = ExtResource("1")
EOF

echo "compat-smoke: $("$godot" --version) against compatibility_minimum $min" >&2
# The first editor pass registers the extension and builds the global class cache. A second
# pass catches what the first one only queued. Their exit status is ignored; errors in the
# log still fail.
for _ in 1 2; do
	timeout 300 "$godot" --headless --path "$proj" --editor --quit >"$work/import.log" 2>&1 || true
done
status=0
timeout 120 "$godot" --headless --path "$proj" --max-fps 60 >"$work/run.log" 2>&1 || status=$?
cat "$work/run.log"
errors=$(grep -E '^(ERROR|SCRIPT ERROR|USER ERROR):' "$work/import.log" "$work/run.log" || true)
if [ -n "$errors" ]; then
	echo "compat-smoke: Godot logged errors:" >&2
	echo "$errors" >&2
	exit 1
fi
if [ "$status" != 0 ] || ! grep -q '^COMPAT-SMOKE-OK$' "$work/run.log"; then
	echo "compat-smoke: failed (exit $status); logs in $work" >&2
	exit 1
fi
echo "compat-smoke: ok on $min" >&2
# A temporary folder is removed on success; failures keep their logs.
[ -n "$keep" ] || rm -rf "${work:?}"
