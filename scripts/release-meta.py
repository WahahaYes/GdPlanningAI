#!/usr/bin/env python3
"""Release metadata shared by scripts/release.sh and the release workflow.

  release-meta.py version       print the version in addons/GdPlanningAI/plugin.cfg
  release-meta.py check X.Y.Z   exit 1 unless plugin.cfg carries X.Y.Z
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

plugin.cfg is the one release version. The Rust crate's version stays 0.0.0:
it is not a release version, so a release never touches Cargo.toml.
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
    m = re.search(r'^version="([^"]*)"', PLUGIN_CFG.read_text(), re.M)
    return m.group(1) if m else ""


def check(ver):
    if plugin_version() != ver:
        print(
            "release-meta: addons/GdPlanningAI/plugin.cfg is not at %s" % ver,
            file=sys.stderr,
        )
        return 1
    return 0


def section(text, heading):
    """(start, end, body) of the "## <heading>" section, or None."""
    m = re.search(
        r"^## %s(?: [^\n]*)?\n(.*?)(?=^## |\Z)" % re.escape(heading), text, re.S | re.M
    )
    return (m.start(), m.end(), m.group(1).strip()) if m else None


def notes(ver):
    found = section(CHANGELOG.read_text(), ver)
    if not found or not found[2]:
        print("release-meta: no CHANGELOG section for %s" % ver, file=sys.stderr)
        return 1
    print(found[2])
    return 0


def changelog(ver, date, notes_file=None):
    text = CHANGELOG.read_text()
    head = "## %s - %s" % (ver, date)
    mine, unreleased = section(text, ver), section(text, "Unreleased")
    if notes_file:
        body = Path(notes_file).read_text().strip()
        if not body:
            print("release-meta: %s is empty" % notes_file, file=sys.stderr)
            return 1
    elif mine and mine[2]:
        print(
            "release-meta: keeping the existing CHANGELOG section for %s" % ver,
            file=sys.stderr,
        )
        return 0
    elif unreleased and unreleased[2]:
        body = unreleased[2]
    else:
        print(
            "release-meta: no notes for %s: write them under ## Unreleased or pass a notes file"
            % ver,
            file=sys.stderr,
        )
        return 1
    new = "%s\n\n%s\n\n" % (head, body)
    # The released section takes the place of this version's old section,
    # else of Unreleased, else goes above the newest release.
    target = mine or unreleased
    if target:
        text = text[: target[0]] + new + text[target[1] :]
    else:
        first = re.search(r"^## ", text, re.M)
        at = first.start() if first else len(text)
        text = text[:at] + new + text[at:]
    # Keep an Unreleased heading on top, so PRs add their notes under it
    # instead of each adding the heading (which conflicts between PRs).
    if not section(text, "Unreleased"):
        first = re.search(r"^## ", text, re.M)
        at = first.start() if first else len(text)
        text = text[:at] + "## Unreleased\n\n" + text[at:]
    CHANGELOG.write_text(text)
    return 0


def stale():
    versions = re.findall(r"^## (\d+\.\d+\.\d+)\b", CHANGELOG.read_text(), re.M)
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
    ).stdout
    for line in out.splitlines():
        print("release-meta: version hardcoded here: %s" % line, file=sys.stderr)
    return 1 if out.strip() else 0


if __name__ == "__main__":
    cmds = {
        "version": (lambda: print(plugin_version()) or 0, 0, 0),
        "check": (check, 1, 1),
        "notes": (notes, 1, 1),
        "changelog": (changelog, 2, 3),
        "stale": (stale, 0, 0),
    }
    args = sys.argv[2:]
    spec = cmds.get(sys.argv[1] if len(sys.argv) > 1 else "")
    if not spec or not (spec[1] <= len(args) <= spec[2]):
        sys.exit(__doc__.strip())
    sys.exit(spec[0](*args))
