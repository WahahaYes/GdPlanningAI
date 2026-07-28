#!/usr/bin/env python3
"""Record Godot scene to video using OBS WebSocket.

Usage:
    OBS_PASSWORD=your_password capture_obs.py examples/hunger_basic_2d.tscn
    capture_obs.py -f -d 15 examples/campfire_2d.tscn

Environment:
    OBS_HOST      WebSocket host (default: localhost)
    OBS_PORT      WebSocket port (default: 4455)
    OBS_PASSWORD  WebSocket password (required)
    DURATION      Recording duration in seconds (default: 10)
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.resolve()))

from obs_controller import OBSController

# ── Helpers ────────────────────────────────────────────────────────────────


def _pgrep_obs() -> bool:
    """Check if OBS is running via pgrep."""
    try:
        return (
            subprocess.run(
                ["pgrep", "-x", "obs"], capture_output=True, check=False
            ).returncode
            == 0
        )
    except FileNotFoundError:
        return False


def ensure_obs_running() -> subprocess.Popen | None:
    """Start OBS if not running.

    Returns Popen handle if *we* launched OBS (caller should kill on cleanup),
    None if OBS was already running.  Exits on failure.
    """
    if _pgrep_obs():
        return None

    for obs_cmd in ["/usr/bin/obs", "/usr/bin/obs-studio", "obs", "obs-studio"]:
        try:
            proc = subprocess.Popen(
                [obs_cmd],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                start_new_session=True,
            )
            print("  OBS starting \u2014 waiting for WebSocket server...")
            return proc
        except (FileNotFoundError, OSError):
            continue

    print("\u2717 Could not start OBS")
    sys.exit(1)


def launch_godot_scene(
    project_dir: Path,
    scene_path: Path,
    fullscreen: bool = False,
    max_fps: int = 60,
    godot_path: str = "godot",
) -> subprocess.Popen:
    """Launch a Godot scene in the background.

    Uses ``--max-fps`` (not ``--fixed-fps``) to cap rendering without
    disabling real-time synchronisation, so physics, animations and
    ``_process(delta)`` all advance at wall-clock speed.
    """
    godot_cmd = [
        godot_path,
        "--path",
        str(project_dir),
        "--scene",
        f"res://{scene_path.relative_to(project_dir)}",
        "--max-fps",
        str(max_fps),
    ]
    if fullscreen:
        godot_cmd.append("-f")

    print(f"Launching: {' '.join(godot_cmd)}")
    proc = subprocess.Popen(
        godot_cmd,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
    )
    print(f"  Godot started (PID {proc.pid})")
    return proc


def wait_for_file(path: Path, timeout: float = 5.0) -> None:
    """Wait for *path* to exist with non-zero size.

    OBS hybrid-fragmented MP4 buffers all frames in a single memory fragment
    and flushes asynchronously *after* ``stop_record()`` returns.  Without
    this wait the output file is still empty when we try to move it.
    """
    if not path.exists():
        return
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if path.stat().st_size > 0:
            return
        time.sleep(0.2)


# ── Main ───────────────────────────────────────────────────────────────────


def main() -> int:
    """Run Godot scene and record via OBS."""
    import argparse

    parser = argparse.ArgumentParser(
        description="OBS-based Godot scene capture",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=(
            "Examples:\n"
            "  OBS_PASSWORD=... %(prog)s examples/hunger_basic_2d.tscn\n\n"
            "  %(prog)s -f -d 15 examples/campfire_2d.tscn\n"
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
        "-f", "--fullscreen", action="store_true", help="Fullscreen mode"
    )
    parser.add_argument(
        "--scene-name",
        help="OBS scene to use for recording (default: current scene)",
    )
    parser.add_argument("--output", "-o", help="Output video path")
    parser.add_argument(
        "--no-quit-wait",
        action="store_true",
        help="Let scene run until natural exit (ignores --duration)",
    )
    parser.add_argument(
        "--start-obs",
        action="store_true",
        help="Start OBS automatically if not running",
    )
    parser.add_argument(
        "--max-fps",
        type=int,
        default=60,
        help="Godot max render FPS (default: 60)",
    )
    parser.add_argument(
        "--godot-path",
        default="godot",
        help="Godot binary path (default: godot from PATH)",
    )

    args = parser.parse_args()

    # ── Validate / resolve paths ────────────────────────────────────────

    project_dir = Path(__file__).parent.parent.resolve()
    scene_path = (project_dir / args.scene_path).resolve()

    if not scene_path.exists():
        print(f"\u2717 Scene not found: {scene_path}")
        return 1

    if not os.getenv("OBS_PASSWORD"):
        print("\u2717 OBS_PASSWORD environment variable required")
        return 1

    output_path: Path
    if args.output:
        output_path = Path(args.output)
    else:
        d = project_dir / "media" / "captures"
        d.mkdir(parents=True, exist_ok=True)
        output_path = d / f"{scene_path.stem}_{time.strftime('%Y%m%d_%H%M%S')}.mp4"

    print(f"Scene: {scene_path}")
    print(f"Duration: {args.duration if not args.no_quit_wait else 'indefinite'}")
    print(f"Output: {output_path}\n")

    # ── Tracked resources ───────────────────────────────────────────────

    obs_proc: subprocess.Popen | None = None  # OBS process *we* launched
    godot_proc: subprocess.Popen | None = None
    obs: OBSController | None = None
    exit_code = 0

    # ── Recording flow (everything inside try so cleanup always runs) ──

    try:
        # 1. Start OBS if requested
        if args.start_obs:
            obs_proc = ensure_obs_running()

        # 2. Connect to OBS WebSocket
        obs = OBSController(password=os.getenv("OBS_PASSWORD"))
        print("  Connecting to OBS WebSocket...")
        obs._wait_for_connection(max_retries=15, delay=2.0)

        # 3. Ensure monitor capture source exists in the scene
        obs.ensure_screen_capture()

        # 4. Launch Godot
        godot_proc = launch_godot_scene(
            project_dir=project_dir,
            scene_path=scene_path,
            fullscreen=args.fullscreen,
            max_fps=args.max_fps,
            godot_path=args.godot_path,
        )
        time.sleep(2)

        # 4.5.  Stop any previous recording that might still be active
        if obs.is_recording():
            print("  Stopping previous OBS recording...")
            obs.stop_recording()
            time.sleep(0.5)

        # 5. Start recording
        obs.start_recording(args.scene_name)

        # 6. Wait for duration or Godot exit (wall-clock)
        if args.no_quit_wait:
            print(
                f"\nRecording \u2014 waiting for Godot (PID {godot_proc.pid}) to exit..."
            )
            godot_proc.wait()
            print(f"  Godot exited with code {godot_proc.returncode}")
        else:
            print(f"\nRecording for {args.duration}s...")
            deadline = time.monotonic() + args.duration
            while time.monotonic() < deadline:
                ret = godot_proc.poll()
                if ret is not None:
                    print(f"  Godot exited early with code {ret}")
                    break
                time.sleep(0.1)
            else:
                godot_proc.kill()
                godot_proc.wait()
                print(f"  {args.duration}s elapsed \u2014 Godot killed")

    except KeyboardInterrupt:
        print("\n\u26a0 Interrupted")
        exit_code = 1
    except Exception as e:  # noqa: BLE001 — CLI top-level handler
        print(f"\u2717 Failed: {e}")
        exit_code = 1
    finally:
        if godot_proc is not None and godot_proc.poll() is None:
            godot_proc.kill()
            godot_proc.wait()

        if obs is not None:
            try:
                stopped_path = obs.stop_recording()
            except Exception:  # noqa: BLE001
                stopped_path = None

            if stopped_path:
                wait_for_file(stopped_path)
                if stopped_path != output_path:
                    try:
                        shutil.move(str(stopped_path), str(output_path))
                        print(f"\u2713 Moved to: {output_path}")
                    except OSError as e:
                        print(f"\u26a0 Could not move output: {e}")
                        print(f"  File saved at: {stopped_path}")

            obs.close()

        # SIGKILL OBS to bypass the "open streams" modal popup.
        if obs_proc is not None and obs_proc.poll() is None:
            obs_proc.kill()
            obs_proc.wait()

    return exit_code


if __name__ == "__main__":
    sys.exit(main())
