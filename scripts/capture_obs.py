#!/usr/bin/env python3
"""
Record Godot scene to video using OBS WebSocket.

This approach uses OBS for screen/window capture with hardware-accelerated encoding,
avoiding the frame rate penalties of Godot's native MovieWriter.

Usage:
    OBS_PASSWORD=your_password python capture_obs.py examples/hunger_basic_2d.tscn

Requirements:
    - OBS Studio running with WebSocket server enabled (Tools → WebSocket Server)
    - `obsws-python` package: uv pip install -e ".[obs]"
    - Godot project must have a window title that OBS can capture (or use window capture)

Environment variables:
    OBS_HOST       - WebSocket host (default: localhost)
    OBS_PORT       - WebSocket port (default: 4455)
    OBS_PASSWORD   - Required: WebSocket password
    DURATION       - Recording duration in seconds (default: 10, overridable via CLI)
"""

from __future__ import annotations

import os
import subprocess
import sys
import time
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    pass

# Add scripts dir to path for obs_controller import
SCRIPTS_DIR = Path(__file__).parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from obs_controller import OBSController  # noqa: E402


def find_godot_window(scene_name: str) -> int | None:
    """Find Godot window ID by scene name using wmctrl."""
    try:
        result = subprocess.run(
            ["wmctrl", "-l", "-G"],
            capture_output=True,
            text=True,
            check=True,
        )
        for line in result.stdout.strip().split("\n"):
            parts = line.split()
            if len(parts) >= 7:
                window_id = parts[0]
                window_title = " ".join(parts[7:])
                if scene_name in window_title or "Godot" in window_title:
                    return int(window_id, 16)
    except (subprocess.CalledProcessError, FileNotFoundError):
        pass
    return None


def main() -> int:
    """Run Godot scene and record via OBS."""
    import argparse

    parser = argparse.ArgumentParser(description="OBS-based Godot scene capture")
    parser.add_argument("scene_path", help="Path to .tscn file")
    parser.add_argument(
        "-d", "--duration", type=int, default=int(os.getenv("DURATION", "10"))
    )
    parser.add_argument("--window-title", "-w", help="Window title pattern to capture")
    parser.add_argument("--scene-name", help="OBS scene to use for recording")
    parser.add_argument("--output", "-o", help="Output video path")
    parser.add_argument(
        "--no-quit-wait", action="store_true", help="Let scene run until natural exit"
    )

    args = parser.parse_args()

    project_dir = Path(__file__).parent.parent
    scene_path = project_dir / args.scene_path

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
    print(f"Duration: {args.duration}s (OBS will record until Godot exits)")
    print(f"Output: {output_path}")
    print()

    # Connect to OBS
    obs = OBSController(password=obs_password)
    try:
        obs.connect()
        # Auto-setup screen capture if needed
        obs.ensure_screen_capture()
    except Exception as e:
        print(f"✗ Failed to connect to OBS: {e}")
        print(
            "   Ensure OBS is running and WebSocket server is enabled (default port 4455)"
        )
        return 1

    # Start recording
    obs.start_recording(args.scene_name)

    # Launch Godot scene
    godot_cmd = [
        "godot",
        "--path",
        str(project_dir),
        "--scene",
        f"res://{args.scene_path}",
        "--fixed-fps",
        "30",  # Match OBS frame rate for smooth capture
    ]

    if not args.no_quit_wait:
        godot_cmd.extend(["--quit-after", str(args.duration * 30)])

    print(f"Running: {' '.join(godot_cmd)}")

    try:
        # Run Godot scene (will block until scene exits)
        proc = subprocess.run(godot_cmd, check=False)
        if proc.returncode != 0:
            print(f"⚠ Godot exited with code {proc.returncode}")
    except KeyboardInterrupt:
        print("\nInterrupted")
    finally:
        # Stop recording regardless
        stopped_path = obs.stop_recording()
        if stopped_path:
            if stopped_path != output_path:
                # Rename/move the file if OBS uses different output path
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
