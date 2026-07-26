#!/usr/bin/env bash
#
# capture_all_showcase.sh — Record footage from all key showcase commits.
#
# Runs capture_scene.sh against each milestone commit + scene combo.
# Outputs numbered clips ready for video editing.
#
# Usage:
#   ./scripts/capture_all_showcase.sh [--dry-run] [--commit-only <hash>]
#
# Options:
#   --dry-run              Print what would be captured without executing
#   --commit-only <hash>   Only capture footage for a specific commit
#   --scene-only <scene>   Only capture a specific scene across all commits
#   --duration <secs>      Override duration for all captures (default: 30)
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
CAPTURE_DIR="${PROJECT_DIR}/media/captures"
DRY_RUN=""
COMMIT_ONLY=""
SCENE_ONLY=""
DURATION=30

# ─── Parse Args ──────────────────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run)       DRY_RUN="--dry-run"; shift ;;
        --commit-only)   COMMIT_ONLY="$2"; shift 2 ;;
        --scene-only)    SCENE_ONLY="$2"; shift 2 ;;
        --duration)      DURATION="$2"; shift 2 ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

# ─── Capture Manifest ───────────────────────────────────────────────────────
# Format: "commit|scene|label"
# commit: git ref to checkout
# scene: scene path relative to project root
# label: human-readable name for the clip

MANIFEST=(
    # Act 1 — The Foundation
    "dc48fe5|examples/hunger_basic_2d.tscn|01_before_pure_gdscript"

    # Act 2 — The Rust Leap
    "3429296|examples/hunger_basic_2d.tscn|02_first_rust_demos_working"
    "7b7b968|examples/hunger_basic_2d.tscn|03_rust_tests_exist"

    # Act 3 — The Rewrites & Breakthrough
    "6947d24|examples/hunger_basic_2d.tscn|04_reimplementation_status"
    "a55ea1f|examples/hunger_basic_2d.tscn|05_rust_suite_passing"
    "a55ea1f|examples/hunger_multi_agent_2d.tscn|05b_multi_agent_passing"

    # Act 4 — The Polish (current state demos)
    "a9bda00|examples/hunger_basic_2d.tscn|06_final_hunger_basic"
    "a9bda00|examples/hunger_multi_agent_2d.tscn|07_final_multi_agent"
    "a9bda00|examples/hunger_stress_test_2d.tscn|08_final_stress_test"
    "a9bda00|examples/campfire_2d.tscn|09_final_campfire_2d"
    "a9bda00|examples/campfire_3d.tscn|10_final_campfire_3d"
)

# ─── Run Captures ───────────────────────────────────────────────────────────

mkdir -p "$CAPTURE_DIR"
TOTAL=${#MANIFEST[@]}
COUNT=0
FAILED=0

echo "=============================================="
echo " GdPlanningAI Showcase Capture"
echo " ${TOTAL} clips to capture"
echo "=============================================="
echo ""

for entry in "${MANIFEST[@]}"; do
    IFS='|' read -r commit scene label <<< "$entry"
    COUNT=$((COUNT + 1))

    # Filter by --commit-only
    if [[ -n "$COMMIT_ONLY" && "$commit" != "$COMMIT_ONLY" ]]; then
        continue
    fi

    # Filter by --scene-only
    if [[ -n "$SCENE_ONLY" && "$scene" != "$SCENE_ONLY" ]]; then
        continue
    fi

    OUTPUT="${CAPTURE_DIR}/${label}.mp4"

    echo "──────────────────────────────────────────────"
    echo "[${COUNT}/${TOTAL}] ${label}"
    echo "  Commit:  ${commit}"
    echo "  Scene:   ${scene}"
    echo "  Output:  ${OUTPUT}"
    echo "──────────────────────────────────────────────"

    if "$SCRIPT_DIR/capture_scene.sh" \
        "$scene" \
        -c "$commit" \
        -d "$DURATION" \
        -o "$OUTPUT" \
        $DRY_RUN; then
        echo "  ✓ Captured"
    else
        echo "  ✗ FAILED"
        FAILED=$((FAILED + 1))
    fi
    echo ""
done

echo "=============================================="
echo " Done. ${COUNT} attempted, ${FAILED} failed."
echo " Clips saved to: ${CAPTURE_DIR}/"
echo "=============================================="
