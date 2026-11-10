"""Keep docs/ARCHITECTURE.md and docs/generated/ in sync.

The four layered diagrams live in ``docs/generated/architecture-*.mmd`` and are
embedded verbatim as fenced `````mermaid`` blocks in ``docs/ARCHITECTURE.md``;
``docs/generated/api-spec.json`` carries a canonical header stub in front of
the ``classes`` array. This script validates the sources and rewrites the
derived copies, so ARCHITECTURE.md never drifts from the .mmd files.

Exit code 0 - sources valid (and derived copies rewritten, unless --check).
Exit code 1 - an invalid source, or drift when run with --check.
"""

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
GENERATED = ROOT / "docs" / "generated"
ARCH = ROOT / "docs" / "ARCHITECTURE.md"
SPEC = GENERATED / "api-spec.json"

HEADER = {
    "spec_version": "1.0",
    "project": "GdPlanningAI",
    "note": "Read-only map merged from the orchestrator interface dump. "
    "Callable/Variant/Dictionary values crossing the Rust bridge are "
    "serialized, so they are documented here as String-typed with examples. "
    "See per-class notes.",
    "bridge_conventions": {
        "Callable": "String-typed bridge shape. "
        'Example: "callable:MyUpdaters:update_hunger"',
        "Variant_property_value": "String-typed bridge shape. "
        'Example: "variant:int:3" or "variant:Vector2(1, 2)"',
        "Dictionary_bridge_dict": "String-typed bridge shape. "
        'Example: "{\\"kind\\": \\"fact\\", \\"fact\\": \\"at_target\\"}"',
        "fact_value_array": 'String-typed bridge shape. Example: "[\\"loc_kitchen\\"]"',
    },
}

# Section title in ARCHITECTURE.md -> diagram file in docs/generated/.
SECTIONS = {
    "Overview": "architecture-overview.mmd",
    "Runtime": "architecture-runtime.mmd",
    "Authoring": "architecture-authoring.mmd",
    "Specs": "architecture-specs.mmd",
}


def canonical_head() -> str:
    """The expected text of api-spec.json from ``{`` through the header stub."""
    return json.dumps(HEADER, indent=2, ensure_ascii=True)[:-2] + ","


def validate_mmd(path: Path, errors: list[str]) -> str | None:
    """Validate one diagram; return its body, or None when invalid."""
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as exc:
        errors.append(f"{path} unreadable: {exc}")
        return None
    if "\r" in text:
        errors.append(f"{path} has CR line endings; use LF")
        return None
    lines = text.splitlines()
    if not lines or lines[0] != "classDiagram":
        errors.append(f"{path} first line must be 'classDiagram'")
        return None
    depth = 0
    for lineno, line in enumerate(lines, start=1):
        if re.match(r"\s*class\s+\S+\s*\{", line):
            depth += 1
        if depth > 0 and "{kind" in line:
            errors.append(
                f"{path}:{lineno} member line holds '{{kind'; "
                "use the paren form 'kind (...)' so mermaid still parses"
            )
        if line.strip() == "}":
            depth = max(0, depth - 1)
    return text.rstrip("\n")


def section_pattern(title: str) -> re.Pattern[str]:
    return re.compile(
        r"(## " + re.escape(title) + r".*?\n```mermaid\n).*?\n(```)",
        re.DOTALL,
    )


def embed_sections(
    arch: str, diagrams: dict[str, str], errors: list[str]
) -> tuple[str, list[str]]:
    """Embed each .mmd body; return the updated text plus drift messages."""
    drifts: list[str] = []
    for title, filename in SECTIONS.items():
        pattern = section_pattern(title)
        match = pattern.search(arch)
        if match is None:
            errors.append(f"{ARCH.name} has no mermaid block under '## {title}'")
            continue
        wanted = match.group(1) + diagrams[filename] + "\n" + match.group(2)
        if wanted != match.group(0):
            drifts.append(f"{ARCH.name} '## {title}' block differs from {filename}")
            arch = arch[: match.start()] + wanted + arch[match.end() :]
    return arch, drifts


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--check",
        action="store_true",
        help="report drift instead of rewriting derived copies",
    )
    check = parser.parse_args().check

    errors: list[str] = []
    diagrams: dict[str, str] = {}
    for filename in SECTIONS.values():
        body = validate_mmd(GENERATED / filename, errors)
        if body is not None:
            diagrams[filename] = body
    if errors or len(diagrams) != len(SECTIONS):
        if len(diagrams) != len(SECTIONS) and not errors:
            errors.append("a diagram failed validation")
        for problem in errors:
            print(f"error: {problem}")
        print(f"generate_docs: {len(errors)} error(s).", file=sys.stderr)
        return 1

    try:
        raw = SPEC.read_text(encoding="utf-8")
        data = json.loads(raw)
    except (OSError, json.JSONDecodeError) as exc:
        print(f"error: {SPEC} unreadable: {exc}")
        print("generate_docs: 1 error(s).", file=sys.stderr)
        return 1
    if not isinstance(data.get("classes"), list) or not data["classes"]:
        print(f"error: {SPEC} misses a non-empty 'classes' array")
        print("generate_docs: 1 error(s).", file=sys.stderr)
        return 1
    anchor = '\n  "classes":'
    idx = raw.find(anchor)
    if idx < 0:
        print(f"error: {SPEC} has no top-level 'classes' key")
        print("generate_docs: 1 error(s).", file=sys.stderr)
        return 1
    head_drift = raw[:idx] != canonical_head()

    try:
        arch = ARCH.read_text(encoding="utf-8")
    except OSError as exc:
        print(f"error: {ARCH} unreadable: {exc}")
        print("generate_docs: 1 error(s).", file=sys.stderr)
        return 1
    updated, drifts = embed_sections(arch, diagrams, errors)
    if errors:
        for problem in errors:
            print(f"error: {problem}")
        print(f"generate_docs: {len(errors)} error(s).", file=sys.stderr)
        return 1
    if head_drift:
        drifts.append(f"{SPEC.name} header stub differs from canonical form")

    if check and drifts:
        for drift in drifts:
            print(f"drift: {drift}")
        print(f"generate_docs: {len(drifts)} drift(s) found.", file=sys.stderr)
        return 1
    if head_drift:
        SPEC.write_text(canonical_head() + raw[idx:], encoding="utf-8")
    if updated != arch:
        ARCH.write_text(updated, encoding="utf-8")
    if head_drift or updated != arch:
        print("generate_docs: rewrote drifted file(s).", file=sys.stderr)
    else:
        print("generate_docs: docs and generated sources agree.", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
