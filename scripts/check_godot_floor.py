"""The oldest supported Godot is stated once and built for.

A GDExtension loads on the Godot API it was built against and newer. The
godot crate's api-4-N feature picks that API; without one, godot-rust uses its
own default, which moves with godot-rust releases (0.5 moved it from 4.5 to
4.6). This check fails when:

- the godot dependency in addons/GdPlanningAI/rust/Cargo.toml has no
  api-4-N feature, or more than one;
- compatibility_minimum in addons/GdPlanningAI/gdplanningai.gdextension is
  missing or differs from that feature.

The store and Asset Library listings carry the minimum too, set by hand;
scripts/release.sh prints it. `make test-compat` runs the addon on it.

Usage:
  uv run scripts/check_godot_floor.py

Prints one necro hit per line (path[:line]: label). Exit code 0 clean, 1 on
any violation, 2 when a file cannot be read.
"""

import re
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CARGO = "addons/GdPlanningAI/rust/Cargo.toml"
GDEXT = "addons/GdPlanningAI/gdplanningai.gdextension"

RE_API = re.compile(r"^api-(\d+)-(\d+)$")
RE_GODOT_DEP = re.compile(r"^\s*godot\s*=")
RE_COMPAT_MIN = re.compile(r'^\s*compatibility_minimum\s*=\s*"([^"]*)"')


def line_of(lines: list[str], pattern: re.Pattern[str]) -> int | None:
    """Returns the 1-based line of the first match, or None."""
    for number, line in enumerate(lines, 1):
        if pattern.search(line):
            return number
    return None


def api_floor(text: str) -> tuple[str | None, str | None]:
    """Returns (version, problem) from the godot dependency's api-4-N features."""
    dep = tomllib.loads(text).get("dependencies", {}).get("godot")
    if dep is None:
        return None, "no godot dependency"
    features = dep.get("features", []) if isinstance(dep, dict) else []
    apis = [m.groups() for f in features if (m := RE_API.match(f))]
    if not apis:
        return None, (
            "godot has no api-4-N feature, so the minimum Godot moves with "
            "godot-rust's default; pin the oldest supported version"
        )
    if len(apis) > 1:
        return None, "godot has more than one api-4-N feature"
    return ".".join(apis[0]), None


def main() -> int:
    try:
        cargo_text = (ROOT / CARGO).read_text(encoding="utf-8")
        gdext_lines = (ROOT / GDEXT).read_text(encoding="utf-8").splitlines()
    except OSError as exc:
        print(f"check_godot_floor: {exc}", file=sys.stderr)
        return 2

    hits: list[str] = []
    version, problem = api_floor(cargo_text)
    if problem:
        dep_line = line_of(cargo_text.splitlines(), RE_GODOT_DEP)
        hits.append(f"{CARGO}{f':{dep_line}' if dep_line else ''}: {problem}")

    compat_line = line_of(gdext_lines, RE_COMPAT_MIN)
    if compat_line is None:
        hits.append(f"{GDEXT}: no compatibility_minimum")
    elif version is not None:
        compat = RE_COMPAT_MIN.match(gdext_lines[compat_line - 1]).group(1)
        if compat != version:
            hits.append(
                f"{GDEXT}:{compat_line}: compatibility_minimum {compat} differs "
                f"from the godot crate's api-{version.replace('.', '-')} in {CARGO}"
            )

    for hit in hits:
        print(hit)
    return 1 if hits else 0


if __name__ == "__main__":
    sys.exit(main())
