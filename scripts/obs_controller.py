#!/usr/bin/env python3
"""
OBS WebSocket controller for Godot scene recording.

Uses environment variables for configuration:
    OBS_HOST      - WebSocket host (default: localhost)
    OBS_PORT      - WebSocket port (default: 4455)
    OBS_PASSWORD  - WebSocket password (required)

Usage:
    # Install with: uv pip install -e ".[obs]"
    # Or: uv add obsws-python

    OBS_PASSWORD=your_password python obs_controller.py --start
    OBS_PASSWORD=your_password python obs_controller.py --stop
"""

from __future__ import annotations

import os
import sys
from pathlib import Path
from typing import TYPE_CHECKING

from obsws_python import ReqClient

if TYPE_CHECKING:
    from obsws_python.types import RecordStatus, GetVersionResponse


class OBSController:
    """Control OBS recording via WebSocket."""

    # Known screen capture input kinds by priority (prefer pipewire on Linux)
    SCREEN_CAPTURE_KINDS = [
        "pipewire-screen-capture-source",  # Linux Wayland
        "xcomposite_screen",  # Linux X11
        "monitor_capture",  # Windows/macOS display capture
    ]

    def __init__(
        self,
        host: str | None = None,
        port: int | None = None,
        password: str | None = None,
        timeout: float = 10.0,
    ) -> None:
        self.host = host or os.getenv("OBS_HOST", "localhost")
        self.port = port or int(os.getenv("OBS_PORT", "4455"))
        self.password = password or os.getenv("OBS_PASSWORD", "")

        if not self.password:
            msg = "OBS_PASSWORD environment variable required"
            raise ValueError(msg)

        self._client: ReqClient | None = None

    def connect(self) -> None:
        """Establish WebSocket connection to OBS."""
        try:
            self._client = ReqClient(
                host=self.host,
                port=self.port,
                password=self.password,
                timeout=self.timeout if hasattr(self, "timeout") else 10.0,
            )
            version = self.get_version()
            print(
                f"✓ Connected to OBS {version.obs_version} (WebSocket {version.obs_web_socket_version})"
            )
        except Exception as e:
            print(f"✗ Failed to connect to OBS at {self.host}:{self.port}: {e}")
            raise

    @property
    def client(self) -> ReqClient:
        if self._client is None:
            raise RuntimeError("Not connected - call connect() first")
        return self._client

    def ensure_screen_capture(self, scene_name: str = "Scene") -> bool:
        """Ensure a screen capture input exists in the specified scene."""
        # Check if we already have a capture source in this scene
        items = self.client.get_scene_item_list(scene_name)
        has_capture = any(
            "capture" in item.get("sourceName", "").lower()
            for item in items.scene_items
        )
        if has_capture:
            return True

        # Find available screen capture kind
        kinds = self.client.get_input_kind_list(unversioned=False)
        input_kinds = kinds.input_kinds
        available_kind = next(
            (k for k in self.SCREEN_CAPTURE_KINDS if k in input_kinds), None
        )

        if not available_kind:
            print("✗ No screen capture input kind available")
            print(f"   Available: {input_kinds}")
            return False

        try:
            self.client.create_input(
                sceneName=scene_name,
                inputName="GodotCapture",
                inputKind=available_kind,
                inputSettings={},
                sceneItemEnabled=True,
            )
            print(f"✓ Created {available_kind} input for screen capture")
            return True
        except Exception as e:
            print(f"⚠ Could not create screen capture: {e}")
            return False

    def get_version(self) -> GetVersionResponse:
        """Get OBS version info."""
        return self.client.get_version()

    def get_record_status(self) -> RecordStatus:
        """Get current recording status."""
        return self.client.get_record_status()

    def is_recording(self) -> bool:
        """Check if OBS is currently recording."""
        status = self.get_record_status()
        return status.output_active

    def start_recording(self, scene_name: str | None = None) -> None:
        """Start recording. Optionally switch to a specific scene first."""
        if scene_name:
            self.set_scene(scene_name)

        self.client.start_record()
        print(
            f"✓ OBS recording started{f' (scene: {scene_name})' if scene_name else ''}"
        )

    def stop_recording(self) -> Path | None:
        """Stop recording and return output path."""
        if not self.is_recording():
            print("⚠ OBS was not recording")
            return None

        response = self.client.stop_record()
        output_path = Path(response.output_path) if response.output_path else None
        if output_path:
            print(f"✓ OBS recording stopped: {output_path}")
        return output_path

    def set_scene(self, scene_name: str) -> bool:
        """Switch to a specific scene. Returns True if successful."""
        try:
            self.client.set_current_program_scene(sceneName=scene_name)
            print(f"✓ Scene changed to: {scene_name}")
            return True
        except Exception as e:
            print(f"✗ Failed to change scene to '{scene_name}': {e}")
            return False

    def list_scenes(self) -> list[str]:
        """List all available scene names."""
        response = self.client.get_scene_list()
        scenes = [
            s["sceneName"] if isinstance(s, dict) else s.sceneName
            for s in response.scenes
        ]
        for scene in scenes:
            print(f"  - {scene}")
        return scenes

    def close(self) -> None:
        """Close the connection."""
        if self._client is not None:
            # ReqClient doesn't expose disconnect, but we can clear reference
            self._client = None


def main() -> int:
    """CLI entry point."""
    import argparse

    parser = argparse.ArgumentParser(description="OBS WebSocket controller")
    parser.add_argument("--host", default=os.getenv("OBS_HOST", "localhost"))
    parser.add_argument("--port", type=int, default=int(os.getenv("OBS_PORT", "4455")))
    parser.add_argument("--password", default=os.getenv("OBS_PASSWORD"))
    parser.add_argument("--scene", "-s", help="Scene to switch to before recording")

    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--start", action="store_true", help="Start recording")
    group.add_argument("--stop", action="store_true", help="Stop recording")
    group.add_argument("--status", action="store_true", help="Show recording status")
    group.add_argument(
        "--list-scenes", action="store_true", help="List available scenes"
    )

    args = parser.parse_args()

    if not args.password:
        print("✗ OBS_PASSWORD is required")
        return 1

    try:
        obs = OBSController(host=args.host, port=args.port, password=args.password)
        obs.connect()

        if args.start:
            obs.start_recording(args.scene)
        elif args.stop:
            obs.stop_recording()
        elif args.status:
            status = obs.get_record_status()
            print(f"Recording: {'active' if status.output_active else 'inactive'}")
        elif args.list_scenes:
            print("Available scenes:")
            obs.list_scenes()

        return 0
    except Exception as e:
        print(f"Error: {e}")
        return 1


if __name__ == "__main__":
    sys.exit(main())
