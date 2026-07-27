#!/usr/bin/env python3
"""
Record Godot scene to video using OBS WebSocket.

This approach uses OBS for screen/window capture with hardware-accelerated encoding,
avoiding the frame rate penalties of Godot's native MovieWriter.

Usage:
    OBS_PASSWORD=your_password python capture_obs.py examples/hunger_basic_2d.tscn

    # Fullscreen mode:
    OBS_PASSWORD=your_password python capture_obs.py -f --duration 15 examples/campfire_2d.tscn

Requirements:
    - OBS Studio running with WebSocket server enabled (Tools → WebSocket Server)
    - `obsws-python` package: uv pip install -e ".[obs]"
    - The Godot project must have a window or screen that OBS can capture

Environment variables:
    OBS_HOST       - WebSocket host (default: localhost)
    OBS_PORT       - WebSocket port (default: 4455)
    OBS_PASSWORD   - Required: WebSocket password
    DURATION       - Recording duration in seconds (default: 10)
"""

from __future__ import annotations

import os
import subprocess
import sys
import time
from pathlib import Path

# Add scripts dir to path for obs_controller import
SCRIPTS_DIR = Path(__file__).parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from obs_controller import OBSController  # noqa: E402


def ensure_obs_running() -> bool:
    """Start OBS if not already running. Returns True if process was launched/alive."""
    try:
        result = subprocess.run(
            ["pgrep", "-x", "obs"], capture_output=True, check=False
        )
        if result.returncode == 0:
            return True
    except FileNotFoundError:
        pass

    obs_paths = ["/usr/bin/obs", "/usr/bin/obs-studio", "obs", "obs-studio"]
    for obs_cmd in obs_paths:
        try:
            subprocess.Popen(
                [obs_cmd],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                start_new_session=True,
            )
            print("  OBS launching — waiting for WebSocket server...")
            return True
        except FileNotFoundError:
            continue
        except Exception:
            continue
    return False


def launch_godot_scene(
    project_dir: Path,
    scene_path: Path,
    fullscreen: bool = False,
    duration: int | None = None,
    fixed_fps: int = 30,
) -> subprocess.Popen:
    """Launch a Godot scene in the background and return the process handle."""
    godot_cmd = [
        "godot",
        "--path",
        str(project_dir),
        "--scene",
        f"res://{scene_path.relative_to(project_dir)}",
        "--fixed-fps",
        str(fixed_fps),
    ]

    if fullscreen:
        godot_cmd.append("-f")

    if duration is not None and duration > 0:
        godot_cmd.extend(["--quit-after", str(duration * fixed_fps)])

    print(f"Launching: {' '.join(godot_cmd)}")
    proc = subprocess.Popen(
        godot_cmd,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
    )
    print(f"  Godot started (PID {proc.pid})")
    return proc


def main() -> int:
    """Run Godot scene and record via OBS."""
    import argparse

    parser = argparse.ArgumentParser(
        description="OBS-based Godot scene capture",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=(
            "Examples:\n"
            "  # Basic monitor capture\n"
            "  OBS_PASSWORD=... %(prog)s examples/hunger_basic_2d.tscn\n\n"
            "  # Fullscreen capture\n"
            "  OBS_PASSWORD=... %(prog)s -f -d 15 examples/campfire_2d.tscn\n"
        ),
    )
    parser.add_argument("scene_path", help="Path to .tscn file")
    parser.add_argument(
        "-d",
        "--duration",
        type=int,
        default=int(os.getenv("DURATION", "10")),
        help="Recording duration in seconds",
    )
    parser.add_argument(
        "-f",
        "--fullscreen",
        action="store_true",
        help="Launch Godot in fullscreen mode",
    )

    parser.add_argument("--scene-name", help="OBS scene to use for recording")
    parser.add_argument("--output", "-o", help="Output video path")
    parser.add_argument(
        "--no-quit-wait",
        action="store_true",
        help="Let scene run until natural exit (ignores --duration)",
    )
    parser.add_argument(
        "--start-obs",
        action="store_true",
        help="Attempt to start OBS if not running",
    )

    args = parser.parse_args()

    # ── Paths ─────────────────────────────────────────────────────────────
    project_dir = Path(__file__).parent.parent.resolve()
    scene_path = (project_dir / args.scene_path).resolve()

    if not scene_path.exists():
        print(f"✗ Scene not found: {scene_path}")
        return 1

    obs_password = os.getenv("OBS_PASSWORD")
    if not obs_password:
        print("✗ OBS_PASSWORD environment variable required")
        print("   Set with: OBS_PASSWORD=your_password ...")
        return 1

    output_dir = project_dir / "media" / "captures"
    output_dir.mkdir(parents=True, exist_ok=True)

    if args.output:
        output_path = Path(args.output)
    else:
        scene_name = scene_path.stem
        timestamp = time.strftime("%Y%m%d_%H%M%S")
        output_path = output_dir / f"{scene_name}_{timestamp}.mp4"

    print(f"Scene: {scene_path}")
    print(f"Duration: {args.duration}s (OBS records until Godot exits)")
    print(f"Output: {output_path}")
    if args.fullscreen:
        print("Mode: fullscreen")
    print()

    # ── Step 1: Start OBS first (takes ~6s to boot WebSocket server) ──────
    if args.start_obs and not ensure_obs_running():
        print("✗ Could not start OBS")
        return 1

    obs = OBSController(password=obs_password)

    # ── Step 2: Wait for OBS WebSocket to be ready ───────────────────────
    if args.start_obs:
        # OBS was just launched — skip the quick attempt, go straight to retry
        print("  Waiting for OBS WebSocket server...")
        obs._wait_for_connection(max_retries=15, delay=2.0)
    else:
        # OBS was already running — try once first, then retry if needed
        try:
            obs.connect()
        except Exception:
            print("  Waiting for OBS WebSocket server...")
            obs._wait_for_connection(max_retries=15, delay=2.0)

    # ── Step 3: Set up screen capture ─────────────────────────────────────
    obs.ensure_screen_capture()

    # ── Step 4: Launch Godot ──────────────────────────────────────────────
    godot_proc = launch_godot_scene(
        project_dir=project_dir,
        scene_path=scene_path,
        fullscreen=args.fullscreen,
        duration=None if args.no_quit_wait else args.duration,
    )
    time.sleep(2)

    # ── Step 5: Start recording ────────────────────────────────────────────
    obs.start_recording(args.scene_name)

    # ── Step 6: Wait for Godot to finish ───────────────────────────────────
    try:
        print(f"\nRecording — waiting for Godot (PID {godot_proc.pid}) to exit...")
        godot_proc.wait()
        print(f"  Godot exited with code {godot_proc.returncode}")
    except KeyboardInterrupt:
        print("\nInterrupted by user")
        godot_proc.kill()

    # ── Step 7: Stop recording ────────────────────────────────────────────
    stopped_path = obs.stop_recording()
    if stopped_path:
        if stopped_path != output_path:
            try:
                stopped_path.rename(output_path)
                print(f"✓ Moved to: {output_path}")
            except Exception as e:
                print(f"⚠ Could not rename output: {e}")
                print(f"  File saved at: {stopped_path}")
    else:
        print("⚠ No output file received from OBS")

    obs.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
