#!/usr/bin/env python3
"""Release metadata shared by scripts/release.sh and the release workflow.

  release-meta.py version       print the version in addons/GdPlanningAI/plugin.cfg
  release-meta.py check X.Y.Z   exit 1 unless plugin.cfg, the Rust crate's Cargo.toml and
                                its Cargo.lock entry all carry X.Y.Z
  release-meta.py bump X.Y.Z    set X.Y.Z in those three files
  release-meta.py notes X.Y.Z   print the CHANGELOG.md section for X.Y.Z;
                                exit 1 when there is none
  release-meta.py changelog X.Y.Z DATE [NOTES_FILE]
                                give CHANGELOG.md a "## X.Y.Z - DATE" section:
                                NOTES_FILE's text if given (replacing an
                                existing X.Y.Z or Unreleased section), else
                                keep an existing X.Y.Z section, else rename
                                "## Unreleased"; exit 1 when none of these
                                yields notes. Leaves an empty "## Unreleased"
                                on top for the next release's notes
  release-meta.py stale         exit 1, listing each hit, if any version in
                                CHANGELOG.md is written into addon source or
                                docs, where a bump would leave it behind

One version, three files: plugin.cfg is the source, and the Rust crate carries the same
version so the built library can report it (GdPAIPlanScheduler.get_engine_version) and the
autoload can warn about a stale binary.
"""

import os
import re
import subprocess
import sys
from pathlib import Path

# RELEASE_ROOT points it at another checkout, such as release.sh's scratch
# worktree.
ROOT = Path(os.environ.get("RELEASE_ROOT") or Path(__file__).resolve().parent.parent)
CHANGELOG = ROOT / "CHANGELOG.md"
PLUGIN_CFG = ROOT / "addons/GdPlanningAI/plugin.cfg"
# Where a hardcoded version would reach users. plugin.cfg and the CHANGELOG
# carry versions on purpose; the lockfile, binaries and Rust tree do not count.
STALE_PATHS = [
    "README.md",
    "docs",
    "addons/GdPlanningAI",
    ":!addons/GdPlanningAI/plugin.cfg",
    ":!addons/GdPlanningAI/bin",
    ":!addons/GdPlanningAI/rust",
    ":!addons/GdPlanningAI/README.md",
]


def plugin_version():
    m = re.search(r'^version="([^"]*)"', PLUGIN_CFG.read_text(), re.MULTILINE)
    return m.group(1) if m else ""


CARGO_TOML = ROOT / "addons/GdPlanningAI/rust/Cargo.toml"
CARGO_LOCK = ROOT / "addons/GdPlanningAI/rust/Cargo.lock"
# (path, pattern whose group 1 is the text before the version, group 2 the version)
VERSION_FILES = [
    (PLUGIN_CFG, r'(^version=")([^"]*)(")'),
    (CARGO_TOML, r'(^version = ")([^"]*)(")'),
    (CARGO_LOCK, r'(name = "gdplanningai-rust"\nversion = ")([^"]*)(")'),
]


def check(ver):
    bad = []
    for path, pattern in VERSION_FILES:
        m = re.search(pattern, path.read_text(), re.MULTILINE)
        if not m or m.group(2) != ver:
            bad.append(path)
    for path in bad:
        print(
            f"release-meta: {path.relative_to(ROOT)} is not at {ver}", file=sys.stderr
        )
    return 1 if bad else 0


def bump(ver):
    for path, pattern in VERSION_FILES:
        text, n = re.subn(
            pattern, rf"\g<1>{ver}\g<3>", path.read_text(), count=1, flags=re.MULTILINE
        )
        if n != 1:
            print(
                f"release-meta: no version found in {path.relative_to(ROOT)}",
                file=sys.stderr,
            )
            return 1
        path.write_text(text)
    return 0


def section(text, heading):
    """(start, end, body) of the "## <heading>" section, or None."""
    m = re.search(
        rf"^## {re.escape(heading)}(?: [^\n]*)?\n(.*?)(?=^## |\Z)",
        text,
        re.DOTALL | re.MULTILINE,
    )
    return (m.start(), m.end(), m.group(1).strip()) if m else None


def notes(ver):
    found = section(CHANGELOG.read_text(), ver)
    if not found or not found[2]:
        print(f"release-meta: no CHANGELOG section for {ver}", file=sys.stderr)
        return 1
    print(found[2])
    return 0


def changelog(ver, date, notes_file=None):
    text = CHANGELOG.read_text()
    head = f"## {ver} - {date}"
    mine, unreleased = section(text, ver), section(text, "Unreleased")
    if notes_file:
        body = Path(notes_file).read_text().strip()
        if not body:
            print(f"release-meta: {notes_file} is empty", file=sys.stderr)
            return 1
    elif mine and mine[2]:
        print(
            f"release-meta: keeping the existing CHANGELOG section for {ver}",
            file=sys.stderr,
        )
        return 0
    elif unreleased and unreleased[2]:
        body = unreleased[2]
    else:
        print(
            f"release-meta: no notes for {ver}: write them under ## Unreleased or pass a notes file",
            file=sys.stderr,
        )
        return 1
    new = f"{head}\n\n{body}\n\n"
    # The released section takes the place of this version's old section,
    # else of Unreleased, else goes above the newest release.
    target = mine or unreleased
    if target:
        text = text[: target[0]] + new + text[target[1] :]
    else:
        first = re.search(r"^## ", text, re.MULTILINE)
        at = first.start() if first else len(text)
        text = text[:at] + new + text[at:]
    # Keep an Unreleased heading on top, so PRs add their notes under it
    # instead of each adding the heading (which conflicts between PRs).
    if not section(text, "Unreleased"):
        first = re.search(r"^## ", text, re.MULTILINE)
        at = first.start() if first else len(text)
        text = text[:at] + "## Unreleased\n\n" + text[at:]
    CHANGELOG.write_text(text)
    return 0


def stale():
    versions = re.findall(r"^## (\d+\.\d+\.\d+)\b", CHANGELOG.read_text(), re.MULTILINE)
    if not versions:
        return 0
    pattern = []
    for v in versions:
        pattern += ["-e", v]
    out = subprocess.run(
        ["git", "-C", str(ROOT), "grep", "-n", "-w", "-F", "-I"]
        + pattern
        + ["--"]
        + STALE_PATHS,
        capture_output=True,
        text=True,
        check=False,
    ).stdout
    for line in out.splitlines():
        print(f"release-meta: version hardcoded here: {line}", file=sys.stderr)
    return 1 if out.strip() else 0


if __name__ == "__main__":
    cmds = {
        "version": (lambda: print(plugin_version()) or 0, 0, 0),
        "check": (check, 1, 1),
        "bump": (bump, 1, 1),
        "notes": (notes, 1, 1),
        "changelog": (changelog, 2, 3),
        "stale": (stale, 0, 0),
    }
    args = sys.argv[2:]
    spec = cmds.get(sys.argv[1] if len(sys.argv) > 1 else "")
    if not spec or not (spec[1] <= len(args) <= spec[2]):
        sys.exit(__doc__.strip())
    sys.exit(spec[0](*args))
