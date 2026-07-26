#!/usr/bin/env bash
#
# capture_scene.sh — Record a Godot scene to video using Godot's built-in MovieWriter.
#
# Usage:
#   ./scripts/capture_scene.sh <scene_path> [options]
#
# Options:
#   -c, --commit <hash>     Git commit to checkout before recording
#   -d, --duration <secs>   Recording duration in seconds (default: 10)
#   -o, --output <path>     Output video path (default: media/captures/<scene>_<timestamp>.avi)
#   -w, --width <px>        Capture width (default: 1920)
#   -h, --height <px>       Capture height (default: 1080)
#   -r, --resolution <WxH>  Shortcut for width+height (e.g. 1280x720)
#   -f, --fps <fps>         Frames per second (default: 30)
#   -n, --no-build          Skip Rust build step
#   -b, --build-only        Build Rust then exit (no recording)
#   --dry-run               Print commands without executing
#   --help                  Show this help
#
# Examples:
#   ./scripts/capture_scene.sh examples/hunger_basic_2d.tscn
#   ./scripts/capture_scene.sh examples/campfire_3d.tscn -c 9e72c76 -d 20
#   ./scripts/capture_scene.sh examples/hunger_multi_agent_2d.tscn -r 1280x720
#
# Dependencies:
#   - godot (via godotenv or PATH)
#   - ffmpeg (for AVI→MP4 conversion, optional)
#

set -euo pipefail

# ─── Defaults ────────────────────────────────────────────────────────────────

SCENE_PATH=""
COMMIT=""
DURATION=10
OUTPUT=""
WIDTH=1920
HEIGHT=1080
FPS=30
NO_BUILD=false
BUILD_ONLY=false
DRY_RUN=false
NO_CONVERT=false
PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
CAPTURE_DIR="${PROJECT_DIR}/media/captures"

# ─── Colors ──────────────────────────────────────────────────────────────────

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

info()  { echo -e "${BLUE}[info]${NC}  $*"; }
ok()    { echo -e "${GREEN}[ok]${NC}    $*"; }
warn()  { echo -e "${YELLOW}[warn]${NC}  $*"; }
err()   { echo -e "${RED}[error]${NC} $*" >&2; }

# ─── Usage ───────────────────────────────────────────────────────────────────

usage() {
    sed -n '/^# Usage:/,/^$/p' "$0" | sed 's/^# \?//'
    exit 0
}

# ─── Parse Args ──────────────────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
    case "$1" in
        -c|--commit)    COMMIT="$2"; shift 2 ;;
        -d|--duration)  DURATION="$2"; shift 2 ;;
        -o|--output)    OUTPUT="$2"; shift 2 ;;
        -w|--width)     WIDTH="$2"; shift 2 ;;
        -h|--height)    HEIGHT="$2"; shift 2 ;;
        -r|--resolution)
            IFS='x' read -r WIDTH HEIGHT <<< "$2"
            shift 2 ;;
        -f|--fps)       FPS="$2"; shift 2 ;;
        -n|--no-build)  NO_BUILD=true; shift ;;
        -b|--build-only) BUILD_ONLY=true; shift ;;
        --no-convert)   NO_CONVERT=true; shift ;;
        --dry-run)      DRY_RUN=true; shift ;;
        --help)         usage ;;
        -*)             err "Unknown option: $1"; usage ;;
        *)
            if [[ -z "$SCENE_PATH" ]]; then
                SCENE_PATH="$1"
            else
                err "Unexpected argument: $1"
                usage
            fi
            shift ;;
    esac
done

if [[ -z "$SCENE_PATH" ]]; then
    err "No scene path specified."
    usage
fi

# ─── Dependency Checks ──────────────────────────────────────────────────────

check_deps() {
    local missing=()

    if ! command -v godot &>/dev/null; then
        missing+=("godot (install via godotenv: https://github.com/nicemicro/godotenv)")
    fi

    if [[ ${#missing[@]} -gt 0 ]]; then
        if $DRY_RUN; then
            warn "Missing dependencies (OK in dry-run mode):"
            for dep in "${missing[@]}"; do warn "  - $dep"; done
        else
            err "Missing dependencies:"
            for dep in "${missing[@]}"; do err "  - $dep"; done
            exit 1
        fi
    fi
}

check_deps

# ─── Resolve Paths ──────────────────────────────────────────────────────────

if [[ "$SCENE_PATH" != res://* ]]; then
    if [[ -f "${PROJECT_DIR}/${SCENE_PATH}" ]]; then
        SCENE_RES_PATH="res://${SCENE_PATH}"
    elif [[ -f "${PROJECT_DIR}/addons/GdPlanningAI/${SCENE_PATH}" ]]; then
        SCENE_RES_PATH="res://addons/GdPlanningAI/${SCENE_PATH}"
    else
        err "Scene not found: ${SCENE_PATH}"
        exit 1
    fi
else
    SCENE_RES_PATH="$SCENE_PATH"
fi

FRAMES=$((DURATION * FPS))

if [[ -z "$OUTPUT" ]]; then
    mkdir -p "$CAPTURE_DIR"
    SCENE_NAME=$(basename "$SCENE_PATH" .tscn)
    TIMESTAMP=$(date +%Y%m%d_%H%M%S)
    OUTPUT="${CAPTURE_DIR}/${SCENE_NAME}_${TIMESTAMP}.mp4"
fi

case "${OUTPUT##*.}" in
    avi|ogv) TEMP_AVI="$OUTPUT"; NO_CONVERT=true ;;
    mp4)     TEMP_AVI="${OUTPUT%.mp4}_raw.avi"; NO_CONVERT=false ;;
    *)       TEMP_AVI="${OUTPUT%.*}_raw.avi"; NO_CONVERT=false; OUTPUT="${OUTPUT%.*}.mp4" ;;
esac

info "Scene:      ${SCENE_RES_PATH}"
info "Duration:   ${DURATION}s (${FRAMES} frames @ ${FPS}fps)"
info "Resolution: ${WIDTH}x${HEIGHT}"
info "Output:     ${OUTPUT}"
[[ -n "$COMMIT" ]] && info "Commit:     ${COMMIT}"
echo ""

# ─── Run Command (respects --dry-run) ───────────────────────────────────────

run() {
    if $DRY_RUN; then
        echo -e "${YELLOW}[dry-run]${NC} $*"
        return 0
    fi
    "$@"
}

# ─── Git Checkout ────────────────────────────────────────────────────────────

ORIGINAL_REF=""

save_and_checkout() {
    if [[ -n "$COMMIT" ]]; then
        ORIGINAL_REF=$(git -C "$PROJECT_DIR" rev-parse --abbrev-ref HEAD 2>/dev/null || git -C "$PROJECT_DIR" rev-parse HEAD)
        info "Saving current ref: ${ORIGINAL_REF}"
        info "Checking out commit: ${COMMIT}"
        run git -C "$PROJECT_DIR" checkout "$COMMIT" --quiet
    fi
}

restore_ref() {
    if [[ -n "$ORIGINAL_REF" ]]; then
        info "Restoring ref: ${ORIGINAL_REF}"
        run git -C "$PROJECT_DIR" checkout "$ORIGINAL_REF" --quiet
    fi
}

trap restore_ref EXIT

save_and_checkout

# ─── Build Rust ──────────────────────────────────────────────────────────────

if ! $NO_BUILD && ! $BUILD_ONLY; then
    info "Building Rust planner..."
    if [[ -f "${PROJECT_DIR}/addons/GdPlanningAI/rust/Makefile" ]]; then
        run make -C "${PROJECT_DIR}/addons/GdPlanningAI/rust" build-release 2>&1 | tail -5
    elif [[ -f "${PROJECT_DIR}/Makefile" ]]; then
        run make -C "$PROJECT_DIR" test-rust 2>&1 | tail -5
    else
        warn "No Makefile found — skipping build."
    fi
    ok "Build complete"
fi

if $BUILD_ONLY; then
    ok "Build-only mode — exiting."
    exit 0
fi

# ─── Import Resources ───────────────────────────────────────────────────────

info "Importing project resources..."
run godot --path "$PROJECT_DIR" --headless --import --quit 2>/dev/null || true
ok "Import complete"

# ─── Record with MovieWriter ────────────────────────────────────────────────

GODOT_BIN=$(which godot)
info "Using Godot: ${GODOT_BIN}"
info "Recording ${DURATION}s of ${WIDTH}x${HEIGHT} @ ${FPS}fps via MovieWriter..."

MOVIE_LOG="/tmp/godot_moviewriter.log"

PROJECT_GODOT="${PROJECT_DIR}/project.godot"
BACKUP_GODOT="${PROJECT_DIR}/project.godot.bak"
cp "$PROJECT_GODOT" "$BACKUP_GODOT"

if grep -q '^\[display\]' "$PROJECT_GODOT"; then
    sed -i "/^\[display\]/,/^\[/ s|viewport_width=.*|viewport_width=${WIDTH}|" "$PROJECT_GODOT"
    sed -i "/^\[display\]/,/^\[/ s|viewport_height=.*|viewport_height=${HEIGHT}|" "$PROJECT_GODOT"
else
    sed -i '/^\[editor_plugins\]/i [display]\nwindow/size/viewport_width='"${WIDTH}"'\nwindow/size/viewport_height='"${HEIGHT}"'' "$PROJECT_GODOT"
fi

restore_project() {
    if [[ -f "$BACKUP_GODOT" ]]; then
        mv "$BACKUP_GODOT" "$PROJECT_GODOT"
    fi
}
trap restore_project EXIT

run godot --path "$PROJECT_DIR" \
    --scene "$SCENE_RES_PATH" \
    --write-movie "$TEMP_AVI" \
    --resolution "${WIDTH}x${HEIGHT}" \
    --fixed-fps "$FPS" \
    --quit-after "$FRAMES" \
    --disable-vsync \
    2>&1 | tee "$MOVIE_LOG"

RECORD_EXIT=${PIPESTATUS[0]}

# ─── Convert AVI → MP4 ─────────────────────────────────────────────────────

if [[ $RECORD_EXIT -eq 0 && -f "$TEMP_AVI" ]]; then
    if [[ "$NO_CONVERT" == true ]]; then
        OUTPUT="$TEMP_AVI"
    elif command -v ffmpeg &>/dev/null; then
        info "Converting AVI → MP4..."
        run ffmpeg -y -i "$TEMP_AVI" \
            -c:v libx264 -preset medium -crf 18 \
            -pix_fmt yuv420p \
            "$OUTPUT" \
            2>/tmp/ffmpeg_convert.log

        CONV_EXIT=${PIPESTATUS[0]}
        if [[ $CONV_EXIT -eq 0 ]]; then
            rm -f "$TEMP_AVI"
        else
            warn "ffmpeg conversion failed — keeping AVI: $TEMP_AVI"
            OUTPUT="$TEMP_AVI"
        fi
    else
        warn "ffmpeg not found — output is AVI: $TEMP_AVI"
        OUTPUT="$TEMP_AVI"
    fi
fi

# ─── Result ──────────────────────────────────────────────────────────────────

if [[ $RECORD_EXIT -eq 0 && -f "$OUTPUT" ]]; then
    FILESIZE=$(du -h "$OUTPUT" | cut -f1)
    ok "Recording saved: ${OUTPUT} (${FILESIZE})"
else
    err "Recording failed (exit code: ${RECORD_EXIT})"
    if [[ -f "$MOVIE_LOG" ]]; then
        err "Godot log:"
        tail -20 "$MOVIE_LOG"
    fi
    exit 1
fi
