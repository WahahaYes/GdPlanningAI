#!/usr/bin/env bash
#
# capture_all_showcase.sh — Record footage from all key showcase commits.
#
# Uses git worktrees to check out each target commit without disturbing
# the working tree, then runs the OBS-based capture_obs.py from the
# worktree directory.
#
# Usage:
#   ./scripts/capture_all_showcase.sh [--dry-run] [--commit-only <hash>]
#
# Options:
#   --dry-run              Print what would be captured without executing
#   --commit-only <hash>   Only capture footage for a specific commit
#   --scene-only <scene>   Only capture a specific scene across all commits
#   --duration <secs>      Override duration for all captures (default: 30)
#   --fps <fps>            Override Godot render FPS (default: 60)
#
# Dependencies:
#   - OBS Studio with WebSocket server enabled (port 4455)
#   - OBS_PASSWORD in .env or environment
#   - godot (via godotenv or PATH)
#

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

# ─── Parse Args ──────────────────────────────────────────────────────────────

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

# ─── Load .env for OBS_PASSWORD ─────────────────────────────────────────────

if [[ -f "${PROJECT_DIR}/.env" ]]; then
    set -a; source "${PROJECT_DIR}/.env"; set +a
fi

if [[ -z "${OBS_PASSWORD:-}" ]]; then
    echo "[error] OBS_PASSWORD is required. Set it in .env or export it."
    exit 1
fi

if [[ ! -x "$VENV_PYTHON" ]]; then
    echo "[error] Virtual environment Python not found at ${VENV_PYTHON}"
    echo "       Run 'uv sync' first."
    exit 1
fi

# ─── godotenv — manage Godot versions per commit ──────────────────────────
# Detect godotenv CLI (manages multiple Godot installations). When available,
# we resolve the per-commit Godot version from project.godot and use the
# matching binary — both for the editor --quit regeneration and the capture.
GODOTENV_CMD=""
for candidate in godotenv "${HOME}/.dotnet/tools/godotenv" "${HOME}/.local/bin/godotenv"; do
    if command -v "$candidate" &>/dev/null; then
        GODOTENV_CMD="$candidate"
        break
    fi
done
if [[ -z "$GODOTENV_CMD" ]]; then
    info "godotenv not found — will use 'godot' from PATH"
fi

# ─── Resolve Godot binary per commit ──────────────────────────────────────
# Extracts the target Godot version from the commit's project.godot and uses
# godotenv to provide the matching binary. Falls back to plain 'godot'.
resolve_godot_bin() {
    local commit="$1"
    local features minor

    features=$(git -C "$PROJECT_DIR" show "$commit:project.godot" 2>/dev/null \
        | grep -oP 'config/features=PackedStringArray\("\K[^"]+' || true)

    if [[ -z "$features" ]]; then
        # Fallback — no version info
        echo "godot"
        return
    fi

    # Extract major.minor (e.g. "4.5" from "4.5" or "4.5" from "4.5.0")
    minor="${features%%.*}.${features#*.}"
    minor="${minor%[^0-9.]*}"

    # godotenv manages version strings like "4.5-stable"
    local ver="${minor}-stable"
    local godotenv_bin

    if [[ -n "$GODOTENV_CMD" ]]; then
        # Check if already installed
        if "$GODOTENV_CMD" godot list 2>/dev/null | grep -q "$ver"; then
            "$GODOTENV_CMD" godot use "$ver" &>/dev/null || true
            godotenv_bin=$("$GODOTENV_CMD" godot env target 2>/dev/null) || true
            if [[ -n "$godotenv_bin" && -x "$godotenv_bin" ]]; then
                echo "$godotenv_bin"
                return
            fi
        fi

        # Try to install
        info "Installing Godot $ver via godotenv..."
        if "$GODOTENV_CMD" godot install "$ver" &>/dev/null; then
            "$GODOTENV_CMD" godot use "$ver" &>/dev/null || true
            godotenv_bin=$("$GODOTENV_CMD" godot env target 2>/dev/null) || true
            if [[ -n "$godotenv_bin" && -x "$godotenv_bin" ]]; then
                echo "$godotenv_bin"
                return
            fi
        else
            info "Could not install Godot $ver — falling back to default"
        fi

        # Fallback to current active version
        godotenv_bin=$("$GODOTENV_CMD" godot env target 2>/dev/null) || true
        if [[ -n "$godotenv_bin" && -x "$godotenv_bin" ]]; then
            echo "$godotenv_bin"
            return
        fi
    fi

    # Final fallback — plain 'godot'
    echo "godot"
}

# Rust (cargo) may not be in PATH — use the standard location as fallback
CARGO="${CARGO:-"${HOME}/.cargo/bin/cargo"}"
if [[ ! -x "$CARGO" ]]; then
    # Last resort: look for cargo on PATH
    CARGO="$(command -v cargo 2>/dev/null || true)"
fi

# ─── Capture Manifest ────────────────────────────────────────────────────────
# Format: "commit|scene|label|act"
# commit: git ref to checkout
# scene:  scene path relative to project root
# label:  human-readable name for the clip
# act:    era subdirectory for organizing output files

MANIFEST=(
    # ── Act 1 — The Foundation ────────────────────────────────────────────────
    # Pure GDScript era. Old scene layout under addons/GdPlanningAI/examples/2D/demo_scenes/.
    "dc48fe5|addons/GdPlanningAI/examples/2D/demo_scenes/single_agent_demo.tscn|01_before_pure_gdscript|act1_foundation"
    "dc48fe5|addons/GdPlanningAI/examples/2D/demo_scenes/multi_agent_demo.tscn|01b_multi_agent|act1_foundation"
    "dc48fe5|addons/GdPlanningAI/examples/2D/demo_scenes/multithreading_stress_test.tscn|01c_stress_test|act1_foundation"

    # ── Act 2 — The Rust Leap ─────────────────────────────────────────────────
    # Early Rust integration. Scene layout under addons/GdPlanningAI/examples/demo_2d/scenes/.
    "3429296|addons/GdPlanningAI/examples/demo_2d/scenes/single_agent_demo.tscn|02_first_rust_demos_working|act2_rust_leap"
    "3429296|addons/GdPlanningAI/examples/demo_2d/scenes/multi_agent_demo.tscn|02b_multi_agent|act2_rust_leap"

    "7b7b968|addons/GdPlanningAI/examples/demo_2d/scenes/single_agent_demo.tscn|03_rust_tests_exist|act2_rust_leap"
    "7b7b968|addons/GdPlanningAI/examples/demo_2d/scenes/multi_agent_demo.tscn|03b_multi_agent|act2_rust_leap"

    # ── Act 3 — The Rewrites & Breakthrough ──────────────────────────────────
    # Modern scene names first appear under examples/.
    "6947d24|examples/hunger_basic_2d.tscn|04_reimplementation_status|act3_rewrites"
    "6947d24|examples/hunger_multi_agent_2d.tscn|04b_multi_agent|act3_rewrites"
    "6947d24|examples/hunger_stress_test_2d.tscn|04c_stress_test|act3_rewrites"

    "a55ea1f|examples/hunger_basic_2d.tscn|05_rust_suite_passing|act3_rewrites"
    "a55ea1f|examples/hunger_multi_agent_2d.tscn|05b_multi_agent_passing|act3_rewrites"
    "a55ea1f|examples/hunger_stress_test_2d.tscn|05c_stress_test|act3_rewrites"

    # ── Act 4 — The Polish ───────────────────────────────────────────────────
    # Current state demos — the full set.
    "a9bda00|examples/hunger_basic_2d.tscn|06_final_hunger_basic|act4_polish"
    "a9bda00|examples/hunger_multi_agent_2d.tscn|07_final_multi_agent|act4_polish"
    "a9bda00|examples/hunger_stress_test_2d.tscn|08_final_stress_test|act4_polish"
    "a9bda00|examples/campfire_2d.tscn|09_final_campfire_2d|act4_polish"
    "a9bda00|examples/campfire_3d.tscn|10_final_campfire_3d|act4_polish"
)

# ─── Helpers ─────────────────────────────────────────────────────────────────

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

# ─── Worktree cleanup on exit ───────────────────────────────────────────────

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

# ─── Print summary ──────────────────────────────────────────────────────────

TOTAL=${#MANIFEST[@]}
echo "=============================================="
echo " GdPlanningAI Showcase Capture"
echo " ${TOTAL} clips via OBS (${FPS}fps)"
echo " Worktrees in: ${WORKTREE_BASE}/"
echo "=============================================="
echo ""

# ─── Run Captures ───────────────────────────────────────────────────────────

mkdir -p "$CAPTURE_DIR"
COUNT=0
FAILED=0

for entry in "${MANIFEST[@]}"; do
    IFS='|' read -r commit scene label act <<< "$entry"
    COUNT=$((COUNT + 1))

    # Filter by --commit-only
    if [[ -n "$COMMIT_ONLY" && "$commit" != "$COMMIT_ONLY" ]]; then
        continue
    fi

    # Filter by --scene-only
    if [[ -n "$SCENE_ONLY" && "$scene" != "$SCENE_ONLY" ]]; then
        continue
    fi

    scene_name="$(basename "$scene" .tscn)"
    OUTPUT_DIR="${CAPTURE_DIR}/${act}"
    OUTPUT="${OUTPUT_DIR}/${label}--${scene_name}--${commit}.mp4"
    WORKTREE_DIR="${WORKTREE_BASE}/${label}"
    mkdir -p "$OUTPUT_DIR"

    echo "──────────────────────────────────────────────"
    echo "[${COUNT}/${TOTAL}] ${label}"
    echo "  Commit:  ${commit}"
    echo "  Scene:   ${scene}"
    echo "  Act:     ${act}"
    echo "  Output:  ${OUTPUT}"
    echo "──────────────────────────────────────────────"

    # Create git worktree for this commit (isolated checkout)
    if [[ -z "$DRY_RUN" ]]; then
        if [[ -d "$WORKTREE_DIR" ]]; then
            git -C "$PROJECT_DIR" worktree remove "$WORKTREE_DIR" --force 2>/dev/null || true
            rm -rf "$WORKTREE_DIR"
        fi
        mkdir -p "$WORKTREE_BASE"
        git -C "$PROJECT_DIR" worktree add "$WORKTREE_DIR" "$commit" --quiet
        WORKTREES_CREATED+=("$WORKTREE_DIR")
        info "Worktree created at ${WORKTREE_DIR}"
    else
        info "Would create worktree at ${WORKTREE_DIR} for ${commit}"
    fi

    # Verify scene file exists in the worktree
    if [[ ! -f "${WORKTREE_DIR}/${scene}" ]]; then
        err "Scene not found in worktree: ${WORKTREE_DIR}/${scene}"
        echo "  Skipping."
        FAILED=$((FAILED + 1))
        continue
    fi

    # Symlink recording scripts into the worktree (don't exist in old commits)
    # Symlinks let capture_obs.py resolve its parent dir to the worktree root,
    # so godot --path finds the correct project.godot.
    if [[ -z "$DRY_RUN" ]]; then
        mkdir -p "$WORKTREE_DIR/scripts"
        ln -sf "$SCRIPT_DIR/capture_obs.py"   "$WORKTREE_DIR/scripts/capture_obs.py"
        ln -sf "$SCRIPT_DIR/obs_controller.py" "$WORKTREE_DIR/scripts/obs_controller.py"
    else
        info "Would symlink scripts into ${WORKTREE_DIR}/scripts/"
    fi

    # Build Rust binary with shared artifact cache and safe parallelism.
    #   - Symlinks the worktree's target/ to `.worktrees/cargo-target/` so
    #     every commit shares the same incremental build cache.
    #   - Uses `cargo build` (debug profile) — no LTO, far less RAM than
    #     `--release`, and the .gdextension loads the same .so either way.
    #   - `CARGO_BUILD_JOBS=2` caps peak memory well under 6.6GB.
    #   - Commits without Rust (e.g. dc48fe5) are skipped automatically.
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
            info "No Rust code at this commit — skipping build"
        fi
    fi

    # Resolve the Godot binary for this commit (via godotenv if available)
    GODOT_BIN=$(resolve_godot_bin "$commit")
    info "Godot binary: ${GODOT_BIN} ($($GODOT_BIN --version 2>/dev/null || echo 'unknown'))"

    # Regenerate .godot/ by running the editor briefly and quitting.
    # A fresh worktree has no .godot/ — without it, global_script_class_cache.cfg
    # is missing, so GDScript class_name declarations are never registered at
    # runtime, and every script referencing types like GdPAIBlackboard fails to
    # parse.  The editor startup scans all .gd files, writes class_name cache +
    # UID cache + extension_list.cfg into .godot/, then quits.  This is the
    # correct per-commit initialization — unlike copying .godot/ from HEAD
    # (which can carry stale extension_list.cfg referencing GDExtensions that
    # didn't exist at older commits, cascading into type-registration failure).
    #
    # Note: the --editor startup also compiles shaders, so first invocation is
    # slow (~2-4s).  This is a one-time cost per worktree.
    if [[ -z "$DRY_RUN" ]]; then
        rm -rf "${WORKTREE_DIR}/.godot"
        info "Regenerating .godot/ via godot --editor --quit..."
        if ! "$GODOT_BIN" --path "$WORKTREE_DIR" --editor --quit &>/dev/null; then
            err "godot --editor --quit failed for ${commit}"
            FAILED=$((FAILED + 1))
            continue
        fi
        ok ".godot/ regenerated"
    else
        info "[dry-run] Would regenerate .godot/ in ${WORKTREE_DIR}"
    fi

    # Record via OBS
    # Use the main repo's venv Python (not uv run) so it works regardless
    # of the worktree's pyproject.toml contents.
    # Pass the resolved Godot binary so capture_obs.py uses the same version.
    if run "$VENV_PYTHON" "$WORKTREE_DIR/scripts/capture_obs.py" \
        "$scene" \
        -d "$DURATION" \
        -o "$OUTPUT" \
        -f \
        --start-obs \
        --max-fps "$FPS" \
        --godot-path "$GODOT_BIN"; then
        echo "  ✓ Captured"
    else
        echo "  ✗ FAILED"
        FAILED=$((FAILED + 1))
    fi
    echo ""
done

# ─── Done ────────────────────────────────────────────────────────────────────

echo "=============================================="
echo " Done. ${COUNT} attempted, ${FAILED} failed."
echo " Clips saved to: ${CAPTURE_DIR}/"
echo "=============================================="
