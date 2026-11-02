# /// script
# requires-python = ">=3.13"
# dependencies = ["gdtoolkit==4.5.0"]
# ///
"""Translate gdformat --check and gdlint findings into necro hit-lines.

Runs the pinned gdtoolkit (`gdtoolkit==4.5.0`, the same pin as uv.lock) over
every tracked `*.gd` file and prints one necro hit per stdout line,
`path[:line[:col]]: label`. Human summaries go to stderr so necro never tries
to parse them. Invoke as `uv run --script scripts/necro_gdtoolkit.py`, the
same shape as necro's own script-rule examples.

Exit code 0: every file formatted and lint-clean.
Exit code 1: one or more hits printed.
Exit code 2: the tools could not run (missing binary, git failure, or
unrecognized output alongside a nonzero tool exit).
"""

import re
import shutil
import subprocess
import sys

RE_GDFORMAT_REFORMAT = re.compile(r"^would reformat (.+)$")
RE_GDLINT_HIT = re.compile(r"^(.+?):(\d+): Error: (.*)$")
RE_PARSE_POS = re.compile(r"at line (\d+), column (\d+)")
RE_SKIP = re.compile(
    r"^(Success: .*|Failure: .*|\d+ files? would be (left unchanged|reformatted).*)$"
)


def git_tracked_gd() -> list[str]:
    try:
        result = subprocess.run(
            ["git", "ls-files", "-z", "--", "*.gd"],
            capture_output=True,
            text=True,
            check=True,
        )
    except (subprocess.CalledProcessError, FileNotFoundError) as exc:
        print(f"necro_gdtoolkit: could not run 'git ls-files': {exc}", file=sys.stderr)
        sys.exit(2)
    return [p for p in result.stdout.split("\0") if p]


def run_tool(name: str, files: list[str]) -> tuple[int, str]:
    exe = shutil.which(name)
    if exe is None:
        print(
            f"necro_gdtoolkit: {name} not on PATH "
            f"(PEP 723 header pins gdtoolkit for `uv run --script`)",
            file=sys.stderr,
        )
        sys.exit(2)
    try:
        args = [exe, *files]
        if name == "gdformat":
            args = [exe, "--check", *files]
        proc = subprocess.run(
            args,
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError as exc:
        print(f"necro_gdtoolkit: could not run {name}: {exc}", file=sys.stderr)
        sys.exit(2)
    return proc.returncode, proc.stdout + proc.stderr


def translate(files: list[str]) -> tuple[list[str], list[str]]:
    """Return (hits, unrecognized) parsed from both tools' combined output."""
    known = set(files)
    hits: list[str] = []
    unrecognized: list[str] = []
    pending_parse_error: str | None = None
    tool_failed = False

    for name in ("gdformat", "gdlint"):
        status, output = run_tool(name, files)
        if status != 0:
            tool_failed = True
        skip_tail = False
        for line in output.splitlines():
            stripped = line.strip()
            if not stripped or RE_SKIP.match(stripped):
                skip_tail = False
                continue
            m = RE_GDFORMAT_REFORMAT.match(stripped)
            if m:
                hits.append(f"{m.group(1)}: needs reformatting (gdformat --check)")
                pending_parse_error = None
                skip_tail = False
                continue
            m = RE_GDLINT_HIT.match(stripped)
            if m:
                hits.append(f"{m.group(1)}:{m.group(2)}: {m.group(3)}")
                pending_parse_error = None
                skip_tail = False
                continue
            if stripped.endswith(":") and stripped[:-1] in known:
                # A leftover marker means the previous block never gave a
                # position: attribute it whole-file rather than drop it.
                if pending_parse_error is not None:
                    hits.append(
                        f"{pending_parse_error}: parse error reported "
                        f"by {name} (no position parsed)"
                    )
                pending_parse_error = stripped[:-1]
                skip_tail = False
                continue
            m = RE_PARSE_POS.search(stripped)
            if m and pending_parse_error is not None:
                hits.append(
                    f"{pending_parse_error}:{m.group(1)}:{m.group(2)}: "
                    f"parse error: {stripped[:160]}"
                )
                pending_parse_error = None
                # What follows (expected-tokens, previous-tokens) is tail
                # context of the hit just emitted.
                skip_tail = True
                continue
            if pending_parse_error is not None or skip_tail:
                # Code context inside a parse-error block; the hit covers it.
                continue
            pending_parse_error = None
            unrecognized.append(f"{name}: {line}")
        if pending_parse_error is not None:
            hits.append(
                f"{pending_parse_error}: parse error reported "
                f"by {name} (no position parsed)"
            )
            pending_parse_error = None

    real_unrecognized = [u for u in unrecognized if u.strip()]
    return hits, (real_unrecognized if tool_failed else [])


def main() -> int:
    files = git_tracked_gd()
    if not files:
        print("necro_gdtoolkit: no tracked *.gd files.", file=sys.stderr)
        return 0
    hits, unrecognized = translate(files)
    if unrecognized and not hits:
        print(
            "necro_gdtoolkit: tools failed with output this script does not "
            "recognize; refusing to guess at attribution:",
            file=sys.stderr,
        )
        for line in unrecognized:
            print(f"  {line}", file=sys.stderr)
        return 2
    for hit in hits:
        print(hit)
    if unrecognized:
        print(
            "necro_gdtoolkit: note: some tool output was not translated:",
            file=sys.stderr,
        )
        for line in unrecognized:
            print(f"  {line}", file=sys.stderr)
    print(
        f"necro_gdtoolkit: {len(hits)} violation(s) in {len(files)} file(s).",
        file=sys.stderr,
    )
    return 1 if hits else 0


if __name__ == "__main__":
    sys.exit(main())
