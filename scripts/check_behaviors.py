"""Check that docs/BEHAVIORS.md and the tests point at each other.

Catalog to tests: every path an entry's ``**Proved by:**`` line names is a tracked file.
Tests to catalog: every planner fixture (``rust/tests/scenarios/<slug>.toml``) is named
after a slug, every example-scene case (``test/scenarios/scenes.cfg``) names one, and every
other file holding tests is listed under some entry. The two runners that execute those
cases are not listed themselves.

Exit code 0 – catalog and tests agree.
Exit code 1 – a dangling slug or path (or git unavailable).
"""

import re
import subprocess
import sys
from pathlib import Path

CATALOG = Path("docs/BEHAVIORS.md")
FIXTURES = "addons/GdPlanningAI/rust/tests/scenarios/"
SCENES = Path("test/scenarios/scenes.cfg")
RUNNERS = {
    "addons/GdPlanningAI/rust/tests/scenarios.rs",
    "test/integration/test_scene_scenarios.gd",
}

RE_SLUG = re.compile(r"^### `([a-z0-9-]+)`", re.MULTILINE)
RE_PROVED = re.compile(r"^\*\*Proved by:\*\*(.*)$", re.MULTILINE)
RE_PATH = re.compile(r"`([^`]+)`")
RE_SCENE_SLUG = re.compile(r'^slug="([^"]*)"', re.MULTILINE)


def git_tracked_files() -> set[str]:
    """Return every git-tracked path."""
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
    return set(result.stdout.splitlines())


def holds_tests(path: str) -> bool:
    """True for a Rust or GUT file that defines tests (fixtures are checked by name)."""
    if re.fullmatch(r"addons/GdPlanningAI/rust/tests/[^/]+\.rs", path):
        return True
    if re.fullmatch(r"test/.*/test_[^/]+\.gd", path):
        return True
    if path.startswith("addons/GdPlanningAI/rust/src/") and path.endswith(".rs"):
        return "#[test]" in Path(path).read_text(encoding="utf-8")
    return False


def main() -> int:
    tracked = git_tracked_files()
    catalog = CATALOG.read_text(encoding="utf-8")
    slugs = set(RE_SLUG.findall(catalog))
    listed = {p for line in RE_PROVED.findall(catalog) for p in RE_PATH.findall(line)}

    problems = [
        f"{CATALOG}: proved by {path}, which is not a tracked file"
        for path in sorted(listed - tracked)
    ]
    for path in sorted(tracked):
        if path.startswith(FIXTURES) and path.endswith(".toml"):
            if Path(path).stem not in slugs:
                problems.append(f"{path}: no catalog entry `{Path(path).stem}`")
        elif path not in RUNNERS and path not in listed and holds_tests(path):
            problems.append(f"{path}: holds tests but no catalog entry lists it")
    if str(SCENES) in tracked:
        for slug in RE_SCENE_SLUG.findall(SCENES.read_text(encoding="utf-8")):
            if slug not in slugs:
                problems.append(
                    f"{SCENES}: case names `{slug}`, which is not a catalog entry"
                )

    for problem in problems:
        print(problem)
    if problems:
        print(f"\ncheck_behaviors: {len(problems)} dangling slug(s) or path(s).")
        return 1
    print("check_behaviors: catalog and tests agree.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
