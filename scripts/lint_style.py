"""Style-guide linter for GdPlanningAI source files.

Uses ``git ls-files`` to enumerate tracked .gd and .rs files so that
third-party addon code is never scanned.

Exit code 0  – no violations found.
Exit code 1  – one or more violations found (or git unavailable).

Checks
------
GDScript
  - gd-walrus   : walrus / type-inference operator ``:=`` used (explicit types required)
  - gd-border   : decorative comment border (``# ---...``)
  - gd-spacing  : fewer than two blank lines between ``func`` definitions
  - gd-export   : ``@export`` variable without a ``##`` docstring on the preceding line

Rust
  - rs-module   : ``.rs`` file missing ``//!`` module-level doc at the top
  - rs-pub-doc  : ``pub`` struct / enum / fn without a ``///`` doc-comment immediately above
  - rs-border   : decorative comment border (``// ---...``)
"""

import re
import subprocess
import sys
from pathlib import Path

BORDER_DASHES = 6  # minimum run of dashes to count as a border


def git_tracked_files(extensions: list[str]) -> list[Path]:
    """Return all git-tracked files whose suffix is in *extensions*."""
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

    paths: list[Path] = []
    for line in result.stdout.splitlines():
        p = Path(line)
        if p.suffix in extensions:
            paths.append(p)
    return paths


def report(violations: list[tuple]) -> int:
    """Print violations and return an exit code."""
    if not violations:
        print("lint_style: no violations found.")
        return 0

    for path, lineno, rule, message in sorted(
        violations, key=lambda v: (str(v[0]), v[1])
    ):
        print(f"{path}:{lineno}: [{rule}] {message}")

    print(f"\nlint_style: {len(violations)} violation(s) found.")
    return 1


RE_GD_WALRUS = re.compile(r":=")
RE_GD_BORDER = re.compile(r"#\s*-{" + str(BORDER_DASHES) + r",}")
RE_GD_FUNC = re.compile(r"^(func |static func )")  # matches only unindented funcs
RE_GD_EXPORT = re.compile(r"^\s*@export\b")
RE_GD_DOCSTRING = re.compile(r"^\s*##")


def lint_gdscript(path: Path) -> list[tuple]:
    violations: list[tuple] = []
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as exc:
        print(f"WARNING: could not read {path}: {exc}", file=sys.stderr)
        return violations

    blank_run = 0  # consecutive blank lines immediately before current line
    prev_func_lineno: int | None = None  # 1-indexed line of the last func keyword

    for i, raw in enumerate(lines):
        lineno = i + 1
        stripped = raw.strip()

        # gd-walrus: walrus / type-inference
        if RE_GD_WALRUS.search(stripped):
            violations.append(
                (path, lineno, "gd-walrus", f"type-inference ':=' used: {stripped!r}")
            )

        # gd-border: decorative comment border
        if RE_GD_BORDER.match(stripped):
            violations.append(
                (path, lineno, "gd-border", f"decorative comment border: {stripped!r}")
            )

        # gd-export: @export without ## on the preceding non-blank line
        if RE_GD_EXPORT.match(raw):
            preceding = lines[i - 1].strip() if i > 0 else ""
            if not RE_GD_DOCSTRING.match(preceding):
                violations.append(
                    (
                        path,
                        lineno,
                        "gd-export",
                        f"@export without '##' docstring above: {stripped!r}",
                    )
                )

        # gd-spacing: track blank-line runs before top-level func definitions
        if stripped == "":
            blank_run += 1
        else:
            if RE_GD_FUNC.match(raw):  # raw (not stripped) ensures top-level only
                if prev_func_lineno is not None and blank_run < 2:
                    violations.append(
                        (
                            path,
                            lineno,
                            "gd-spacing",
                            f"only {blank_run} blank line(s) before 'func' "
                            f"(expected 2, previous func at line {prev_func_lineno})",
                        )
                    )
                prev_func_lineno = lineno
            blank_run = 0

    return violations


RE_RS_BORDER = re.compile(r"//\s*-{" + str(BORDER_DASHES) + r",}")
RE_RS_MODULE_DOC = re.compile(r"^//!")
RE_RS_PUB_ITEM = re.compile(r"^pub\s+(struct|enum|fn|async fn)\b")
RE_RS_DOC_COMMENT = re.compile(r"^\s*///")
RE_RS_ATTR_OR_DERIVE = re.compile(r"^\s*#\[")


def lint_rust(path: Path) -> list[tuple]:
    violations: list[tuple] = []
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as exc:
        print(f"WARNING: could not read {path}: {exc}", file=sys.stderr)
        return violations

    # rs-module: first non-empty line should be //!
    first_content = next((line.strip() for line in lines if line.strip()), "")
    if first_content and not RE_RS_MODULE_DOC.match(first_content):
        violations.append(
            (
                path,
                1,
                "rs-module",
                "file does not start with a '//!' module doc comment",
            )
        )

    for i, raw in enumerate(lines):
        lineno = i + 1
        stripped = raw.strip()

        # rs-border: decorative comment border
        if RE_RS_BORDER.match(stripped):
            violations.append(
                (path, lineno, "rs-border", f"decorative comment border: {stripped!r}")
            )

        # rs-pub-doc: pub struct/enum/fn without /// immediately above
        if RE_RS_PUB_ITEM.match(stripped):
            # Walk backwards over attribute lines and blank lines to find doc comment
            j = i - 1
            while j >= 0 and (
                RE_RS_ATTR_OR_DERIVE.match(lines[j].strip()) or lines[j].strip() == ""
            ):
                j -= 1
            preceding = lines[j].strip() if j >= 0 else ""
            if not RE_RS_DOC_COMMENT.match(preceding):
                violations.append(
                    (
                        path,
                        lineno,
                        "rs-pub-doc",
                        f"public item without '///' doc comment above: {stripped!r}",
                    )
                )

    return violations


def main() -> int:
    gd_files = git_tracked_files([".gd"])
    rs_files = git_tracked_files([".rs"])

    violations: list[tuple] = []

    for path in gd_files:
        violations.extend(lint_gdscript(path))

    for path in rs_files:
        violations.extend(lint_rust(path))

    return report(violations)


if __name__ == "__main__":
    sys.exit(main())
