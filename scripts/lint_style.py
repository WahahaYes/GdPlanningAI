"""Style-guide linter for GdPlanningAI source files.

Uses ``git ls-files`` to enumerate tracked .gd and .rs files so that
third-party addon code is never scanned.

Exit code 0  – no violations found.
Exit code 1  – one or more violations found (or git unavailable).

Checks
------
GDScript
  - gd-walrus          : walrus / type-inference operator ``:=`` used (explicit types required)
  - gd-border          : decorative comment border (``# ---...``)
  - gd-spacing         : fewer than two blank lines between ``func`` definitions
  - gd-export          : ``@export`` variable without a ``##`` docstring on the preceding line
  - gd-inline-lambda  : any lambda literal (``func(``) inside a dict/array/paren;
                         predefine as a ``var name: Callable = func(...)`` instead
  - gd-doc-length     : ``##`` block longer than 10 lines (class header) or 6 (anything else)
  - gd-effect-spec    : Action under ``examples/`` (or ``goto_action.gd``) overriding
                         ``simulate_effect`` without a non-empty ``get_effect_spec``
Rust
  - rs-module   : ``.rs`` file missing ``//!`` module-level doc at the top
  - rs-pub-doc  : ``pub`` struct / enum / fn without a ``///`` doc-comment immediately above
  - rs-border   : decorative comment border (``// ---...``)
  - rs-doc-length : doc block longer than 10 lines (``//!`` or type header) or 6 (anything else)
"""

import re
import subprocess
import sys
from pathlib import Path

BORDER_DASHES = 6  # minimum run of dashes to count as a border
HEADER_DOC_MAX = 10  # max lines in a file / class / type doc block
ITEM_DOC_MAX = 6  # max lines in any other doc block


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


def doc_length_violations(
    path: Path, lines: list[str], is_doc, is_header, rule: str
) -> list[tuple]:
    """Flag contiguous doc-comment runs longer than the header or item limit."""
    violations: list[tuple] = []
    i = 0
    while i < len(lines):
        if not is_doc(lines[i]):
            i += 1
            continue
        j = i
        while j < len(lines) and is_doc(lines[j]):
            j += 1
        limit = HEADER_DOC_MAX if is_header(i, j) else ITEM_DOC_MAX
        if j - i > limit:
            violations.append(
                (path, i + 1, rule, f"doc block is {j - i} lines (max {limit})")
            )
        i = j
    return violations


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
RE_GD_NAMED_FUNC = re.compile(r"^(?:static\s+)?func\s+(\w+)\s*\(")
RE_GD_INLINE_LAMBDA = re.compile(r"\bfunc\s*\(")  # any func( usage (lambda)
RE_GD_EXPORT = re.compile(r"^\s*@export\b")
RE_GD_DOCSTRING = re.compile(r"^\s*##")
RE_GD_COMMENT = re.compile(r"^##?")  # unindented only, mirrors RE_GD_FUNC
RE_GD_DOC = re.compile(r"^\s*##(?!#)")
RE_GD_PREAMBLE = re.compile(r"^(@tool|@icon|class_name |extends |##|$)")
RE_GD_CLASS = re.compile(r"^\s*class ")


def lint_gdscript(path: Path) -> list[tuple]:
    violations: list[tuple] = []
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as exc:
        print(f"WARNING: could not read {path}: {exc}", file=sys.stderr)
        return violations

    prev_block_lineno: int | None = None  # 1-indexed line of the last func block start
    nesting_depth: int = 0  # tracks { ( [ nesting to detect nested funcs

    for i, raw in enumerate(lines):
        lineno = i + 1
        stripped = raw.strip()
        line_depth = nesting_depth  # depth at start of this line
        in_str = False
        str_ch = ""
        ci = 0
        while ci < len(raw):
            ch = raw[ci]
            if in_str:
                if ch == "\\":
                    ci += 2
                    continue
                if ch == str_ch:
                    in_str = False
            else:
                if ch in ('"', "'"):
                    in_str = True
                    str_ch = ch
                elif ch == "#":
                    break
                elif ch in ("{("):
                    nesting_depth += 1
                elif ch in ("})"):
                    nesting_depth = max(0, nesting_depth - 1)
                elif ch == "[":
                    nesting_depth += 1
                elif ch == "]":
                    nesting_depth = max(0, nesting_depth - 1)
            ci += 1

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

        # gd-inline-lambda: any lambda literal inside a dict/array/paren.
        # gdformat corrupts files that contain inline lambdas; predefine as a named Callable.
        if line_depth > 0 and RE_GD_INLINE_LAMBDA.search(stripped):
            violations.append(
                (
                    path,
                    lineno,
                    "gd-inline-lambda",
                    f"lambda literal inside dict/array/paren: {stripped!r}",
                )
            )

        # gd-spacing: check blank lines before top-level func blocks.
        # A "block" starts at the func keyword or the top of any directly
        # attached ##/# comment run (no blanks between comments and func).
        if RE_GD_FUNC.match(raw) and line_depth == 0:
            block_start = i
            j = i - 1
            while j >= 0 and RE_GD_COMMENT.match(lines[j]):
                block_start = j
                j -= 1

            blank_run = 0
            k = block_start - 1
            while k >= 0 and lines[k].strip() == "":
                blank_run += 1
                k -= 1

            block_lineno = block_start + 1  # 1-indexed
            if prev_block_lineno is not None and blank_run < 2:
                violations.append(
                    (
                        path,
                        block_lineno,
                        "gd-spacing",
                        (
                            f"only {blank_run} blank line(s) before func block "
                            f"(expected 2, previous block at line {prev_block_lineno})"
                        ),
                    )
                )
            prev_block_lineno = block_lineno

    # gd-doc-length: the class doc (file preamble or inner class) gets the header limit
    preamble_end = next(
        (k for k, line in enumerate(lines) if not RE_GD_PREAMBLE.match(line.strip())),
        len(lines),
    )

    def gd_is_header(start: int, end: int) -> bool:
        return end <= preamble_end or (
            end < len(lines) and bool(RE_GD_CLASS.match(lines[end]))
        )

    violations.extend(
        doc_length_violations(
            path,
            lines,
            lambda line: bool(RE_GD_DOC.match(line)),
            gd_is_header,
            "gd-doc-length",
        )
    )

    return violations


RE_RS_BORDER = re.compile(r"//\s*-{" + str(BORDER_DASHES) + r",}")
RE_RS_MODULE_DOC = re.compile(r"^//!")
RE_RS_PUB_ITEM = re.compile(r"^pub\s+(struct|enum|fn|async fn)\b")
RE_RS_DOC_COMMENT = re.compile(r"^\s*///")
RE_RS_ATTR_OR_DERIVE = re.compile(r"^\s*#\[")
RE_RS_ANY_DOC = re.compile(r"^\s*//[/!](?!/)")
RE_RS_TYPE_ITEM = re.compile(
    r"^\s*(pub(\([^)]*\))?\s+)?(struct|enum|trait|union|impl)\b"
)


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

    # rs-doc-length: module docs and type docs get the header limit
    def rs_is_header(start: int, end: int) -> bool:
        if RE_RS_MODULE_DOC.match(lines[start].strip()):
            return True
        k = end
        while k < len(lines) and RE_RS_ATTR_OR_DERIVE.match(lines[k]):
            k += 1
        return k < len(lines) and bool(RE_RS_TYPE_ITEM.match(lines[k]))

    violations.extend(
        doc_length_violations(
            path,
            lines,
            lambda line: bool(RE_RS_ANY_DOC.match(line)),
            rs_is_header,
            "rs-doc-length",
        )
    )

    return violations


def lint_effect_spec(paths: list[Path]) -> list[tuple]:
    """Every Action under examples/ or goto_action.gd that overrides simulate_effect must
    also override get_effect_spec with a non-empty spec, so scenes never silently fall
    back to GDScript effect callbacks. User-land custom actions (outside examples/) are
    exempt: they may keep the base empty spec and its callable fallback.

    This is a healthy-scene guarantee: a conditional `return []` when a referenced object
    is gone (e.g. add_fuel/pickup guarding on `is_instance_valid`) is intentional and
    allowed, since planning around a freed object must fall back to the callable. Only an
    unconditional empty spec is rejected. The authoritative zero-fallback check is the
    dynamic large-scene test (test_effect_ir_zero_fallback.gd), which requires no
    ApplyEffect callbacks while IR covers every action."""
    violations: list[tuple] = []
    for path in paths:
        name = path.name
        text = str(path).replace("\\", "/")
        if not (
            "/examples/" in text
            or text.startswith("examples/")
            or name == "goto_action.gd"
        ):
            continue
        try:
            lines = path.read_text(encoding="utf-8").splitlines()
        except OSError as exc:
            print(f"WARNING: could not read {path}: {exc}", file=sys.stderr)
            continue

        funcs = {}
        for i, raw in enumerate(lines):
            match = RE_GD_NAMED_FUNC.match(raw)
            if match:
                funcs[match.group(1)] = i
        if "simulate_effect" not in funcs:
            continue
        if "get_effect_spec" not in funcs:
            violations.append(
                (
                    path,
                    funcs["simulate_effect"] + 1,
                    "gd-effect-spec",
                    (
                        "overrides simulate_effect but has no get_effect_spec "
                        "(native effect spec required under examples/)"
                    ),
                )
            )
            continue

        start = funcs["get_effect_spec"] + 1
        body: list[str] = []
        for raw in lines[start:]:
            stripped = raw.strip()
            if stripped == "" or stripped.startswith("#"):
                continue
            if not raw[:1].isspace():
                break
            # Strip inline comments and doc lines; spec bodies hold no `#` inside strings.
            code_part = stripped.split("#", 1)[0].strip()
            if code_part == "" or code_part.startswith("##"):
                continue
            body.append(code_part)
        code = [line for line in body if not line.startswith("##")]
        normalized = ["".join(line.split()) for line in code]
        if "".join(normalized) == "return[]":
            violations.append(
                (
                    path,
                    funcs["get_effect_spec"] + 1,
                    "gd-effect-spec",
                    (
                        "get_effect_spec is empty (return []): "
                        "simulate_effect overrides need a non-empty native spec"
                    ),
                )
            )
    return violations


def main() -> int:
    gd_files = git_tracked_files([".gd"])
    rs_files = git_tracked_files([".rs"])

    violations: list[tuple] = []

    for path in gd_files:
        violations.extend(lint_gdscript(path))

    violations.extend(lint_effect_spec(gd_files))

    for path in rs_files:
        violations.extend(lint_rust(path))

    return report(violations)


if __name__ == "__main__":
    sys.exit(main())
