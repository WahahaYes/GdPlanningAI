#!/usr/bin/env python3
"""
Style checker for GdPlanningAI GDScript source files.

This script runs regex rules to ensure the codebase adheres to our style guidelines.  
It exits with a non-zero status if any violations are found.

Currently enforced rules:
1. Multi-parameter function signatures must span multiple lines.
2. Every function must declare an explicit return type.
3. No function with multiple parameters may omit a return type.
4. No source line may exceed 100 characters.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path
from typing import Iterable, List, Tuple

ROOT_DIR = Path(__file__).resolve().parent.parent  # repo root
FILE_EXTS = {".gd"}

# (description, regex, multiline)
REGEX_RULES: List[Tuple[str, re.Pattern[str], bool]] = [
    (
        "Multi-parameter functions on single line",
        re.compile(r"func\s+\w+\([^)]*,[^)]*\)", re.MULTILINE),
        False,
    ),
    (
        "Functions without explicit return types",
        re.compile(r"func\s+\w+\([^)]*\)\s*:\s*$", re.MULTILINE),
        True,
    ),
    (
        "Multi-parameter functions without return types",
        re.compile(r"func\s+\w+\([^)]*,[^)]*\)\s*:\s*$", re.MULTILINE),
        True,
    ),
]

LINE_LENGTH_LIMIT = 100


class Violation:
    def __init__(self, path: Path, line_no: int, rule: str, line: str):
        self.path = path
        self.line_no = line_no
        self.rule = rule
        self.line = line.rstrip("\n")

    def __str__(self) -> str:
        return f"{self.path}:{self.line_no}: [{self.rule}] {self.line}"


def iter_source_files(root: Path) -> Iterable[Path]:
    for p in root.rglob("*"):
        if p.suffix in FILE_EXTS and p.is_file():
            yield p


def check_file(path: Path) -> List[Violation]:
    text = path.read_text(encoding="utf-8", errors="ignore")
    violations: List[Violation] = []

    # Line-length rule (simple line-by-line check)
    for idx, line in enumerate(text.splitlines(), 1):
        if len(line) > LINE_LENGTH_LIMIT:
            violations.append(
                Violation(path, idx, "Line > 100 chars", line)
            )

    # Regex-based rules (file-level)
    for rule_desc, pattern, multiline in REGEX_RULES:
        if multiline:
            # For multiline patterns we rely on re.MULTILINE flag already set.
            for match in pattern.finditer(text):
                # Determine line number of match start
                line_no = text.count("\n", 0, match.start()) + 1
                snippet = match.group(0).replace("\n", " ")
                violations.append(Violation(path, line_no, rule_desc, snippet))
        else:
            for idx, line in enumerate(text.splitlines(), 1):
                if pattern.search(line):
                    violations.append(Violation(path, idx, rule_desc, line))
    return violations


def main() -> None:
    all_violations: List[Violation] = []
    for source_file in iter_source_files(ROOT_DIR):
        all_violations.extend(check_file(source_file))

    if all_violations:
        print("Style violations detected:\n")
        for v in all_violations:
            print(v)
        print(f"\nTotal violations: {len(all_violations)}")
        sys.exit(1)

    print("All files pass style checks.")


if __name__ == "__main__":
    main()
