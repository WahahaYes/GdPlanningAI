"""Godot data files carry no comments.

Flags every full-line `;` comment in tracked .tscn, .tres, .godot, .cfg,
.gdextension and .import files. The header Godot itself writes at the top of
project.godot is allowed. A `;` inside a multi-line string value is not a
comment and is skipped.

Usage:
  uv run scripts/check_godot_comments.py

Prints one necro hit per line (path:line: label). Exit code 0 clean, 1 on any
comment.
"""

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PATTERNS = ["*.tscn", "*.tres", "*.godot", "*.cfg", "*.gdextension", "*.import"]
GODOT_HEADER = [
    "; Engine configuration file.",
    "; It's best edited using the editor UI and not directly,",
    "; since the parameters that go here are not all obvious.",
    ";",
    "; Format:",
    ";   [section] ; section goes between []",
    ";   param=value ; assign values to parameters",
]


def tracked_files() -> list[str]:
    """Returns the tracked Godot data files, relative to the repo root."""
    out = subprocess.run(
        ["git", "ls-files", "-z", "--", *PATTERNS],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    return [p for p in out.split("\0") if p]


def unescaped_quotes(line: str) -> int:
    """Returns the number of double quotes in line not preceded by a backslash."""
    count, escaped = 0, False
    for char in line:
        if escaped:
            escaped = False
        elif char == "\\":
            escaped = True
        elif char == '"':
            count += 1
    return count


def comment_lines(path: str, lines: list[str]) -> list[int]:
    """Returns the 1-based numbers of full-line comments outside string values."""
    start = 0
    if (
        Path(path).name == "project.godot"
        and lines[: len(GODOT_HEADER)] == GODOT_HEADER
    ):
        start = len(GODOT_HEADER)
    hits, in_string = [], False
    for number, line in enumerate(lines[start:], start + 1):
        if not in_string and line.lstrip().startswith(";"):
            hits.append(number)
        if unescaped_quotes(line) % 2:
            in_string = not in_string
    return hits


def main() -> int:
    """Prints a hit per comment line and returns the exit code."""
    found = False
    for path in tracked_files():
        file = ROOT / path
        if not file.is_file():
            continue
        lines = file.read_text(encoding="utf-8", errors="replace").splitlines()
        for number in comment_lines(path, lines):
            print(f"{path}:{number}: comment in a Godot data file")
            found = True
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main())
