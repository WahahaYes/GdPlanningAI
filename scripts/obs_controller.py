#!/usr/bin/env python3
"""
OBS WebSocket controller for Godot scene recording.

Environment:
    OBS_HOST      WebSocket host (default: localhost)
    OBS_PORT      WebSocket port (default: 4455)
    OBS_PASSWORD  WebSocket password (required)
"""

from __future__ import annotations

import json
import os
import time
from pathlib import Path
from typing import TYPE_CHECKING, Any

from obsws_python import ReqClient

if TYPE_CHECKING:
    from obsws_python.types import GetVersionResponse, RecordStatus

# Path for persisting RestoreTokens so they survive source recreation
TOKEN_DIR = Path(__file__).resolve().parent
TOKEN_FILE = TOKEN_DIR / ".obs_capture_token.json"

INPUT_NAME = "GodotCapture"


class OBSController:
    """Control OBS recording via WebSocket.

    Manages PipeWire monitor capture with RestoreToken persistence,
    allowing automated fullscreen recording setup on Wayland.
    """

    # Known screen capture input kinds by priority (prefer pipewire on Linux)
    SCREEN_CAPTURE_KINDS: tuple[str, ...] = (
        "pipewire-screen-capture-source",
        "xcomposite_screen",
        "monitor_capture",
    )

    # Known RestoreToken for full-monitor capture.
    # Obtained from xdg-desktop-portal when user selected "Monitor source".
    _MONITOR_TOKEN = "aa49d601-3119-47a0-a5e5-095f99b35036"

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

    # ── Token persistence ─────────────────────────────────────────────────

    @staticmethod
    def _token_file_path() -> Path:
        return TOKEN_FILE

    def _user_token_path(self) -> Path:
        """User-level token storage (survives project deletion or repo wipe)."""
        return Path.home() / ".config" / "gdplanningai-obs" / "capture_token"

    def load_monitor_token(self) -> str:
        """Load monitor RestoreToken with fallback chain.

        Priority: user config dir → project token file → hardcoded fallback.
        This ensures the token survives project moves, repo wipes, and OBS scene resets.
        """
        # 1. User config dir (most persistent)
        p = self._user_token_path()
        if p.exists():
            try:
                token = p.read_text().strip()
                if token and len(token) > 4:
                    return token
            except OSError:
                pass

        # 2. Project-local token file
        path = self._token_file_path()
        if path.exists():
            try:
                data = json.loads(path.read_text())
                token = data.get("monitor")
                if token and len(token) > 4:
                    return token
            except (json.JSONDecodeError, OSError):
                pass

        # 3. Hardcoded fallback (last resort)
        return self._MONITOR_TOKEN

    def persist_monitor_token(self, token: str) -> None:
        """Save monitor RestoreToken to user config dir AND project file."""
        # User config (primary — survives repo operations)
        config_path = self._user_token_path()
        config_path.parent.mkdir(parents=True, exist_ok=True)
        config_path.write_text(token + "\n")

        # Project-local (secondary — lives alongside the project)
        path = self._token_file_path()
        data: dict[str, Any] = {}
        if path.exists():
            try:
                data = json.loads(path.read_text())
            except (json.JSONDecodeError, OSError):
                pass
        data["monitor"] = token
        path.write_text(json.dumps(data, indent=2) + "\n")

        print(f"✓ Monitor RestoreToken persisted ({token[:16]}...)")

    def _read_active_token(self) -> str | None:
        """Read the current RestoreToken from the OBS GodotCapture source."""
        try:
            if self._input_exists(INPUT_NAME):
                s = self.client.get_input_settings(INPUT_NAME)
                token = s.input_settings.get("RestoreToken", "")
                if token and len(token) > 4:
                    return token
        except Exception:  # noqa: BLE001, S110
            pass
        return None

    # ── Connection ────────────────────────────────────────────────────────

    def _wait_for_connection(self, max_retries: int = 12, delay: float = 2.0) -> None:
        """Retry connection until OBS WebSocket is ready.  Silent on retries."""
        last_error = None
        for attempt in range(max_retries):
            try:
                self._client = ReqClient(
                    host=self.host,
                    port=self.port,
                    password=self.password,
                    timeout=5.0,
                )
                version = self.get_version()
                print(
                    f"✓ Connected to OBS {version.obs_version} "
                    f"(WebSocket {version.obs_web_socket_version})"
                )
                return
            except Exception as e:  # noqa: BLE001 — retry loop
                last_error = e
                if attempt < max_retries - 1:
                    time.sleep(delay)
        msg = f"Failed to connect after {max_retries} attempts: {last_error}"
        raise ConnectionError(msg)

    @property
    def client(self) -> ReqClient:
        if self._client is None:
            raise RuntimeError("Not connected - call connect() first")
        return self._client

    # ── Screen capture source management ──────────────────────────────────

    def _find_available_capture_kind(self) -> str | None:
        """Find the first available screen capture input kind."""
        kinds = self.client.get_input_kind_list(unversioned=False)
        return next(
            (k for k in self.SCREEN_CAPTURE_KINDS if k in kinds.input_kinds),
            None,
        )

    def _input_exists(self, name: str = INPUT_NAME) -> bool:
        """Check if an input with the given name exists."""
        inputs = self.client.get_input_list()
        return any(i.get("inputName") == name for i in inputs.inputs)

    def _has_capture_in_scene(self, scene_name: str) -> bool:
        """Check if a screen-capture-like source exists in the scene."""
        items = self.client.get_scene_item_list(scene_name)
        return any(
            "capture" in item.get("sourceName", "").lower()
            for item in items.scene_items
        )

    def ensure_screen_capture(self, scene_name: str = "Scene") -> bool:
        """Ensure a monitor capture source exists in the scene.

        If the source already exists (from a previous OBS session), reads
        back its RestoreToken and persists it.  If no source exists, creates
        one using the best available token from the fallback chain, then
        reads the resulting token back and persists it.

        This auto-persist loop keeps the token fresh across OBS restarts
        and scene wipes.
        """
        if self._has_capture_in_scene(scene_name):
            token = self._read_active_token()
            if token:
                self.persist_monitor_token(token)
            return True

        available_kind = self._find_available_capture_kind()
        if not available_kind:
            print("✗ No screen capture input kind available")
            return False

        token = self.load_monitor_token()
        settings: dict[str, Any] = {"RestoreToken": token} if token else {}

        try:
            self.client.create_input(
                sceneName=scene_name,
                inputName=INPUT_NAME,
                inputKind=available_kind,
                inputSettings=settings,
                sceneItemEnabled=True,
            )
            kind_label = available_kind.replace("-source", "").replace("_", " ")
            print(f"✓ Created {kind_label}" + (" with saved token" if token else ""))
        except Exception as e:  # noqa: BLE001
            print(f"\u26a0 Could not create screen capture: {e}")
            return False

        # Give the portal a moment to settle, then read back the active token
        time.sleep(0.5)
        active_token = self._read_active_token()
        if active_token:
            self.persist_monitor_token(active_token)
        return True

    # ── Recording control ─────────────────────────────────────────────────

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
        except Exception as e:  # noqa: BLE001
            print(f"✗ Failed to change scene to '{scene_name}': {e}")
            return False

    def close(self) -> None:
        """Close the connection."""
        if self._client is not None:
            self._client = None
