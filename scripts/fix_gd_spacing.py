"""Fix function-block spacing in GDScript files.

Enforces two rules across all git-tracked .gd files:

1. Exactly two blank lines must precede every top-level function block.
   A "function block" begins at the first ``##`` / ``#`` comment or
   docstring that is immediately attached (no blank lines between it
   and the ``func`` / ``static func`` keyword), or at the ``func``
   keyword itself when there is no such comment.

2. Zero blank lines are allowed between a ``##`` / ``#`` comment block
   and the ``func`` / ``static func`` it annotates.  Any blank lines
   between the last comment line and the ``func`` line are removed.

The script is idempotent: running it twice produces no further changes.
It rewrites files in-place and prints a summary of what was changed.
"""

import re
import subprocess
import sys
from pathlib import Path

RE_FUNC = re.compile(r"^(static\s+)?func\s+")
RE_COMMENT = re.compile(r"^##?")  # unindented only, mirrors RE_FUNC


def _build_depth_map(lines: list[str]) -> list[int]:
    """Return per-line nesting depth (0 = top-level) based on {([})] counts.

    The depth recorded for line *i* is the depth at the *start* of that line,
    before processing its characters.  A top-level ``func`` will have depth 0.
    """
    depth = 0
    depths: list[int] = []
    for line in lines:
        depths.append(depth)
        in_string = False
        string_char = ""
        i = 0
        while i < len(line):
            ch = line[i]
            if in_string:
                if ch == "\\":
                    i += 2
                    continue
                if ch == string_char:
                    in_string = False
            else:
                if ch in ('"', "'"):
                    in_string = True
                    string_char = ch
                elif ch == "#":
                    break  # rest of line is a comment
                elif ch in ("{("):
                    depth += 1
                elif ch in ("})"):
                    depth = max(0, depth - 1)
                elif ch == "[":
                    depth += 1
                elif ch == "]":
                    depth = max(0, depth - 1)
            i += 1
    return depths


def git_tracked_gd_files() -> list[Path]:
    try:
        result = subprocess.run(
            ["git", "ls-files"],
            capture_output=True,
            text=True,
            check=True,
        )
    except (subprocess.CalledProcessError, FileNotFoundError) as exc:
        print(f"ERROR: could not run 'git ls-files': {exc}", file=sys.stderr)
        sys.exit(1)

    return [Path(line) for line in result.stdout.splitlines() if line.endswith(".gd")]


def is_blank(line: str) -> bool:
    return line.strip() == ""


def is_comment(line: str) -> bool:
    return RE_COMMENT.match(line) is not None


def is_func(line: str) -> bool:
    return RE_FUNC.match(line) is not None


def fix_spacing(lines: list[str]) -> list[str]:
    """Return a new line list with corrected spacing around top-level funcs.

    Pass 1: Remove blank lines that sit between a comment block and the
            ``func`` / ``static func`` it annotates.

    Pass 2: Ensure exactly two blank lines precede each top-level function
            block (the block starts at the earliest attached comment line).
    """
    lines = _collapse_comment_func_gap(lines)
    lines = _enforce_two_blanks_before_func_block(lines)
    return lines


def _collapse_comment_func_gap(lines: list[str]) -> list[str]:
    """Remove blank lines between a comment block and its func definition.

    Scans for a ``func`` / ``static func`` at column 0 at depth 0, then
    walks back through blank lines; if a ``##`` / ``#`` comment is found
    before hitting a non-blank non-comment line, the intervening blanks
    are removed.
    """
    result = list(lines)
    depths = _build_depth_map(result)
    i = 0
    while i < len(result):
        if is_func(result[i]) and depths[i] == 0:
            j = i - 1
            blank_indices: list[int] = []
            while j >= 0 and is_blank(result[j]):
                blank_indices.append(j)
                j -= 1
            if j >= 0 and is_comment(result[j]):
                for idx in blank_indices:
                    result[idx] = None  # type: ignore[call-overload]
        i += 1

    return [ln for ln in result if ln is not None]


def _enforce_two_blanks_before_func_block(lines: list[str]) -> list[str]:
    """Ensure exactly two blank lines precede every top-level func block.

    A "func block" starts at:
    - the ``func`` / ``static func`` line itself, OR
    - the first ``##`` / ``#`` comment that is directly attached (no
      blank lines between it and the ``func``).

    After pass 1 the comment-to-func gap is already zero, so we only
    need to find the top of the comment run and ensure exactly two blank
    lines appear before it.

    We do not touch functions that are the very first content in the file
    (nothing above them, or only the file header before them).
    """
    result = list(lines)
    changed = True
    while changed:
        changed = False
        depths = _build_depth_map(result)
        i = 0
        while i < len(result):
            if not is_func(result[i]) or depths[i] != 0:
                i += 1
                continue

            block_start = i
            j = i - 1
            while j >= 0 and is_comment(result[j]):
                block_start = j
                j -= 1

            above = j

            if above < 0:
                i += 1
                continue

            blank_count = 0
            k = above
            while k >= 0 and is_blank(result[k]):
                blank_count += 1
                k -= 1

            if k < 0 and blank_count == above + 1:
                i += 1
                continue

            if blank_count == 2:
                i += 1
                continue

            insert_at = block_start
            if blank_count > 2:
                excess = blank_count - 2
                removed = 0
                r = above
                while r >= 0 and removed < excess:
                    if is_blank(result[r]):
                        result.pop(r)
                        removed += 1
                        r -= 1
                    else:
                        break
            else:
                needed = 2 - blank_count
                for _ in range(needed):
                    result.insert(insert_at, "")

            changed = True
            break

    return result


def process_file(path: Path) -> bool:
    """Fix spacing in *path*.  Return True if the file was modified."""
    try:
        original_text = path.read_text(encoding="utf-8")
    except OSError as exc:
        print(f"WARNING: could not read {path}: {exc}", file=sys.stderr)
        return False

    original_lines = original_text.splitlines()
    fixed_lines = fix_spacing(original_lines)

    if fixed_lines == original_lines:
        return False

    fixed_text = "\n".join(fixed_lines)
    if original_text.endswith("\n"):
        fixed_text += "\n"

    try:
        path.write_text(fixed_text, encoding="utf-8")
    except OSError as exc:
        print(f"WARNING: could not write {path}: {exc}", file=sys.stderr)
        return False

    return True


def main() -> int:
    files = git_tracked_gd_files()
    modified: list[Path] = []

    for path in files:
        if process_file(path):
            modified.append(path)

    if not modified:
        print("fix_gd_spacing: no files needed changes.")
        return 0

    for path in modified:
        print(f"  fixed: {path}")
    print(f"\nfix_gd_spacing: {len(modified)} file(s) updated.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
