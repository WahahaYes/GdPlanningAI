#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2025 Ethan Wilson
# https://github.com/WahahaYes/GdPlanningAI

# Refreshes the project's .godot import and class-name caches with a headless import.
# A nonzero exit after a finished import (editor layout loaded, class and extension caches
# written) warns; one before that fails and prints the log. Run from the project root.
# Usage: scripts/godot-import.sh [LOG]   (LOG: where the import output goes; default a temp file)
set -uo pipefail

log=${1:-$(mktemp "${TMPDIR:-/tmp}/godot-import.XXXXXX")}
status=0
godot --headless --path . --import >"$log" 2>&1 || status=$?
[ "$status" = 0 ] && exit 0
if grep -qE 'DONE.*loading_editor_layout' "$log" &&
	[ -s .godot/global_script_class_cache.cfg ] && [ -s .godot/extension_list.cfg ]; then
	echo "${GITHUB_ACTIONS:+::warning title=Godot exited nonzero after import::}godot-import: exit $status after a finished import"
	exit 0
fi
cat "$log"
echo "${GITHUB_ACTIONS:+::error::}godot-import: import failed (exit $status) before it finished; log at $log"
exit 1
