"""Fix gd-spacing violations in tracked GDScript files.

Inserts missing blank lines before ``func`` / ``static func`` definitions so
that each is preceded by exactly two blank lines (unless it is the first
function in the file, or is inside a class/inner-block where a lower blank
count is intentional — see note below).

Note: the rule only applies between top-level and class-body functions.
Functions that are the *first* item after a class header or at the top of the
file are left alone (no preceding func to space from).

Exit code 0  – all files already correct or successfully fixed.
Exit code 1  – one or more files could not be read/written.
"""

import re
import subprocess
import sys
from pathlib import Path

RE_FUNC = re.compile(r"^(func |static func )")  # matches only unindented funcs


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
    return [Path(p) for p in result.stdout.splitlines() if p.endswith(".gd")]


def fix_spacing(lines: list[str]) -> tuple[list[str], int]:
    """Return (fixed_lines, number_of_insertions)."""
    out: list[str] = []
    insertions = 0
    prev_func_idx: int | None = None  # index in *out* of last func line

    for raw in lines:
        if RE_FUNC.match(raw):  # raw (not stripped) ensures top-level only
            if prev_func_idx is not None:
                # Count trailing blank lines already in out
                blank_run = 0
                j = len(out) - 1
                while j >= 0 and out[j].strip() == "":
                    blank_run += 1
                    j -= 1
                needed = max(0, 2 - blank_run)
                for _ in range(needed):
                    out.append("\n")
                insertions += needed
            prev_func_idx = len(out)

        out.append(raw)

    return out, insertions


def main() -> int:
    files = git_tracked_gd_files()
    errors = 0
    total_insertions = 0

    for path in files:
        try:
            text = path.read_text(encoding="utf-8")
        except OSError as exc:
            print(f"ERROR reading {path}: {exc}", file=sys.stderr)
            errors += 1
            continue

        lines = text.splitlines(keepends=True)
        fixed, insertions = fix_spacing(lines)

        if insertions == 0:
            continue

        try:
            path.write_text("".join(fixed), encoding="utf-8")
            print(f"fixed {path} (+{insertions} blank line(s))")
            total_insertions += insertions
        except OSError as exc:
            print(f"ERROR writing {path}: {exc}", file=sys.stderr)
            errors += 1

    if total_insertions:
        print(
            f"\nfix_gd_spacing: inserted {total_insertions} blank line(s) across {len(files)} file(s)."
        )
    else:
        print("fix_gd_spacing: no changes needed.")

    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
