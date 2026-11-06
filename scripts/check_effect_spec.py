"""Effect-spec check for GdPlanningAI's example Actions, a necro rule (necro.yaml).

Prints one necro hit-line per violation, ``path:line: message``, and exits 0;
necro decides pass or fail. The style checks this script once held now come
from necronomicon's packs (necro:gdscript-style, necro:rust-style).
"""

import re
import subprocess
import sys
from pathlib import Path

RE_GD_NAMED_FUNC = re.compile(r"^(?:static\s+)?func\s+(\w+)\s*\(")


def tracked_gd_files() -> list[Path]:
    """Every git-tracked ``.gd`` file."""
    result = subprocess.run(
        ["git", "ls-files", "*.gd"], capture_output=True, text=True, check=True
    )
    return [Path(line) for line in result.stdout.splitlines()]


def effect_spec_violations(paths: list[Path]) -> list[tuple]:
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
                    (
                        "get_effect_spec is empty (return []): "
                        "simulate_effect overrides need a non-empty native spec"
                    ),
                )
            )
    return violations


def main() -> int:
    for path, lineno, message in sorted(
        effect_spec_violations(tracked_gd_files()), key=lambda v: (str(v[0]), v[1])
    ):
        print(f"{path}:{lineno}: {message}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
