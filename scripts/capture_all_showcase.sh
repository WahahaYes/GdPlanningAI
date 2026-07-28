#!/usr/bin/env bash
#
# capture_all_showcase.sh — Record footage from showcase commits via OBS.
# Uses git worktrees to check out each commit without touching the working tree.
#
# Usage: ./capture_all_showcase.sh [--dry-run] [--commit-only <hash>]
#                                 [--scene-only <scene>] [--duration <s>] [--fps <n>]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
WORKTREE_BASE="${PROJECT_DIR}/.worktrees"
CAPTURE_DIR="${PROJECT_DIR}/media/captures"
VENV_PYTHON="${PROJECT_DIR}/.venv/bin/python"

DRY_RUN=""
COMMIT_ONLY=""
SCENE_ONLY=""
DURATION=30
FPS=60

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run)       DRY_RUN="--dry-run"; shift ;;
        --commit-only)   COMMIT_ONLY="$2"; shift 2 ;;
        --scene-only)    SCENE_ONLY="$2"; shift 2 ;;
        --duration)      DURATION="$2"; shift 2 ;;
        --fps)           FPS="$2"; shift 2 ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

if [[ -f "${PROJECT_DIR}/.env" ]]; then
    set -a; source "${PROJECT_DIR}/.env"; set +a
fi

if [[ -z "${OBS_PASSWORD:-}" ]]; then
    echo "[error] OBS_PASSWORD is required. Set it in .env or export it."
    exit 1
fi

if [[ ! -x "$VENV_PYTHON" ]]; then
    echo "[error] Virtual environment Python not found at ${VENV_PYTHON}. Run 'uv sync' first."
    exit 1
fi

# godotenv detection
GODOTENV_CMD=""
for candidate in godotenv "${HOME}/.dotnet/tools/godotenv" "${HOME}/.local/bin/godotenv"; do
    if command -v "$candidate" &>/dev/null; then
        GODOTENV_CMD="$candidate"
        break
    fi
done

# Resolve the Godot binary for a commit by reading its config/features from
# project.godot and matching it to an installed godotenv version.
resolve_godot_bin() {
    local commit="$1" features minor

    features=$(git -C "$PROJECT_DIR" show "$commit:project.godot" 2>/dev/null \
        | grep -oP 'config/features=PackedStringArray\("\K[^"]+' || true)

    if [[ -z "$features" ]]; then
        echo "godot"
        return
    fi

    minor="${features%%.*}.${features#*.}"
    minor="${minor%[^0-9.]*}"
    local ver="${minor}-stable"

    if [[ -n "$GODOTENV_CMD" ]]; then
        if "$GODOTENV_CMD" godot list 2>/dev/null | grep -q "$ver"; then
            "$GODOTENV_CMD" godot use "$ver" &>/dev/null || true
            local bin; bin=$("$GODOTENV_CMD" godot env target 2>/dev/null) || true
            if [[ -n "$bin" && -x "$bin" ]]; then echo "$bin"; return; fi
        fi
        info "Installing Godot $ver via godotenv..."
        if "$GODOTENV_CMD" godot install "$ver" &>/dev/null; then
            "$GODOTENV_CMD" godot use "$ver" &>/dev/null || true
            local bin; bin=$("$GODOTENV_CMD" godot env target 2>/dev/null) || true
            if [[ -n "$bin" && -x "$bin" ]]; then echo "$bin"; return; fi
        else
            info "Could not install Godot $ver \u2014 falling back to default"
        fi
        local bin; bin=$("$GODOTENV_CMD" godot env target 2>/dev/null) || true
        if [[ -n "$bin" && -x "$bin" ]]; then echo "$bin"; return; fi
    fi
    echo "godot"
}

CARGO="${CARGO:-"${HOME}/.cargo/bin/cargo"}"
if [[ ! -x "$CARGO" ]]; then
    CARGO="$(command -v cargo 2>/dev/null || true)"
fi

# Manifest format: commit|scene_path|label|act
MANIFEST=(
    # Act 1 — Foundation (pure GDScript)
    "dc48fe5|addons/GdPlanningAI/examples/2D/demo_scenes/single_agent_demo.tscn|01_before_pure_gdscript|act1_foundation"
    "dc48fe5|addons/GdPlanningAI/examples/2D/demo_scenes/multi_agent_demo.tscn|01b_multi_agent|act1_foundation"
    "dc48fe5|addons/GdPlanningAI/examples/2D/demo_scenes/multithreading_stress_test.tscn|01c_stress_test|act1_foundation"
    # Act 2 — Rust leap
    "3429296|addons/GdPlanningAI/examples/demo_2d/scenes/single_agent_demo.tscn|02_first_rust_demos_working|act2_rust_leap"
    "3429296|addons/GdPlanningAI/examples/demo_2d/scenes/multi_agent_demo.tscn|02b_multi_agent|act2_rust_leap"
    "7b7b968|addons/GdPlanningAI/examples/demo_2d/scenes/single_agent_demo.tscn|03_rust_tests_exist|act2_rust_leap"
    "7b7b968|addons/GdPlanningAI/examples/demo_2d/scenes/multi_agent_demo.tscn|03b_multi_agent|act2_rust_leap"
    # Act 3 — Rewrites & breakthrough
    "6947d24|examples/hunger_basic_2d.tscn|04_reimplementation_status|act3_rewrites"
    "6947d24|examples/hunger_multi_agent_2d.tscn|04b_multi_agent|act3_rewrites"
    "6947d24|examples/hunger_stress_test_2d.tscn|04c_stress_test|act3_rewrites"
    "a55ea1f|examples/hunger_basic_2d.tscn|05_rust_suite_passing|act3_rewrites"
    "a55ea1f|examples/hunger_multi_agent_2d.tscn|05b_multi_agent_passing|act3_rewrites"
    "a55ea1f|examples/hunger_stress_test_2d.tscn|05c_stress_test|act3_rewrites"
    # Act 4 — Polish (current state)
    "a9bda00|examples/hunger_basic_2d.tscn|06_final_hunger_basic|act4_polish"
    "a9bda00|examples/hunger_multi_agent_2d.tscn|07_final_multi_agent|act4_polish"
    "a9bda00|examples/hunger_stress_test_2d.tscn|08_final_stress_test|act4_polish"
    "a9bda00|examples/campfire_2d.tscn|09_final_campfire_2d|act4_polish"
    "a9bda00|examples/campfire_3d.tscn|10_final_campfire_3d|act4_polish"
)

info()  { echo -e "\033[0;34m[info]\033[0m  $*"; }
ok()    { echo -e "\033[0;32m[ok]\033[0m    $*"; }
err()   { echo -e "\033[0;31m[error]\033[0m $*" >&2; }

run() {
    if [[ -n "$DRY_RUN" ]]; then
        echo -e "\033[1;33m[dry-run]\033[0m $*"
        return 0
    fi
    "$@"
}

WORKTREES_CREATED=()
cleanup_worktrees() {
    for wt in "${WORKTREES_CREATED[@]}"; do
        if [[ -d "$wt" ]]; then
            info "Removing worktree: $wt"
            git -C "$PROJECT_DIR" worktree remove "$wt" --force 2>/dev/null || true
        fi
    done
}
trap cleanup_worktrees EXIT

TOTAL=${#MANIFEST[@]}
echo "=============================================="
echo " GdPlanningAI Showcase Capture"
echo " ${TOTAL} clips via OBS (${FPS}fps)"
echo " Worktrees in: ${WORKTREE_BASE}/"
echo "=============================================="
echo ""

mkdir -p "$CAPTURE_DIR"
COUNT=0
FAILED=0

for entry in "${MANIFEST[@]}"; do
    IFS='|' read -r commit scene label act <<< "$entry"
    COUNT=$((COUNT + 1))

    [[ -n "$COMMIT_ONLY" && "$commit" != "$COMMIT_ONLY" ]] && continue
    [[ -n "$SCENE_ONLY" && "$scene" != "$SCENE_ONLY" ]] && continue

    scene_name="$(basename "$scene" .tscn)"
    OUTPUT_DIR="${CAPTURE_DIR}/${act}"
    OUTPUT="${OUTPUT_DIR}/${label}--${scene_name}--${commit}.mp4"
    WORKTREE_DIR="${WORKTREE_BASE}/${label}"
    mkdir -p "$OUTPUT_DIR"

    echo "──────────────────────────────────────────────"
    echo "[${COUNT}/${TOTAL}] ${label}  (commit ${commit})"
    echo "  Scene:   ${scene}"
    echo "  Output:  ${OUTPUT}"
    echo "──────────────────────────────────────────────"

    if [[ -z "$DRY_RUN" ]]; then
        if [[ -d "$WORKTREE_DIR" ]]; then
            git -C "$PROJECT_DIR" worktree remove "$WORKTREE_DIR" --force 2>/dev/null || true
            rm -rf "$WORKTREE_DIR"
        fi
        mkdir -p "$WORKTREE_BASE"
        git -C "$PROJECT_DIR" worktree add "$WORKTREE_DIR" "$commit" --quiet
        WORKTREES_CREATED+=("$WORKTREE_DIR")
    fi

    if [[ ! -f "${WORKTREE_DIR}/${scene}" ]]; then
        err "Scene not found: ${WORKTREE_DIR}/${scene}"
        FAILED=$((FAILED + 1))
        continue
    fi

    if [[ -z "$DRY_RUN" ]]; then
        mkdir -p "$WORKTREE_DIR/scripts"
        ln -sf "$SCRIPT_DIR/capture_obs.py"   "$WORKTREE_DIR/scripts/capture_obs.py"
        ln -sf "$SCRIPT_DIR/obs_controller.py" "$WORKTREE_DIR/scripts/obs_controller.py"
    fi

    # Build Rust if it exists at this commit (shared cargo target, debug profile, jobs=2)
    if [[ -z "$DRY_RUN" ]]; then
        RUST_DIR="${WORKTREE_DIR}/addons/GdPlanningAI/rust"
        if [[ -f "${RUST_DIR}/Cargo.toml" ]]; then
            SHARED_TARGET="${PROJECT_DIR}/.worktrees/cargo-target"
            mkdir -p "$SHARED_TARGET"
            rm -rf "${RUST_DIR}/target"
            ln -sfn "$SHARED_TARGET" "${RUST_DIR}/target"

            info "Building Rust (debug, jobs=2)..."
            CARGO_BUILD_JOBS=2 \
            "$CARGO" build --manifest-path "${RUST_DIR}/Cargo.toml" 2>&1 | tail -5
            mkdir -p "${RUST_DIR}/../bin/linux"
            cp "${SHARED_TARGET}/debug/libgdplanningai_rust.so" \
               "${RUST_DIR}/../bin/linux/libgdplanningai_rust.so" 2>/dev/null || true
            ok "Rust built (debug)"
        else
            info "No Rust code at this commit \u2014 skipping build"
        fi
    fi

    GODOT_BIN=$(resolve_godot_bin "$commit")

    # Regenerate .godot/ via editor --quit (fresh worktree has no .godot/).
    # Without this, global_script_class_cache.cfg is missing and class_name
    # declarations fail at runtime.
    if [[ -z "$DRY_RUN" ]]; then
        rm -rf "${WORKTREE_DIR}/.godot"
        info "Regenerating .godot/ via godot --editor --quit..."
        if ! "$GODOT_BIN" --path "$WORKTREE_DIR" --editor --quit &>/dev/null; then
            err "godot --editor --quit failed for ${commit}"
            FAILED=$((FAILED + 1))
            continue
        fi
        ok ".godot/ regenerated"
    fi

    if run "$VENV_PYTHON" "$WORKTREE_DIR/scripts/capture_obs.py" \
        "$scene" \
        -d "$DURATION" \
        -o "$OUTPUT" \
        -f \
        --start-obs \
        --max-fps "$FPS" \
        --godot-path "$GODOT_BIN"; then
        echo "  \u2713 Captured"
    else
        echo "  \u2717 FAILED"
        FAILED=$((FAILED + 1))
    fi
    echo ""
done

echo "=============================================="
echo " Done. ${COUNT} attempted, ${FAILED} failed."
echo " Clips saved to: ${CAPTURE_DIR}/"
echo "=============================================="
