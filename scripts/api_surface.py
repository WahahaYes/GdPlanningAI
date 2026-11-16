# SPDX-License-Identifier: Apache-2.0
# Copyright 2025 Ethan Wilson
# https://github.com/WahahaYes/GdPlanningAI

"""Write the addon's public API to docs/API.md, read from the code.

Public means every global class the addon defines and the plugin's autoload:
GDScript classes with a class_name (and the script the plugin registers as an
autoload), and the Rust GodotClasses. For each it lists the base class, then
the signals, enums with their values, constants, exported and public
variables, and functions whose names don't start with an underscore
(constructors with parameters included). For Rust, the #[func] and #[export]
members.

Usage:
  uv run scripts/api_surface.py           rewrite docs/API.md
  uv run scripts/api_surface.py --check   report when docs/API.md is stale

--check prints one necro hit (docs/API.md:1: label). Exit code 0 clean, 1
when stale.
"""

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ADDON = ROOT / "addons" / "GdPlanningAI"
RUST_SRC = ADDON / "rust" / "src"
OUT = ROOT / "docs" / "API.md"

PREAMBLE = """# API

The public surface of GdPlanningAI: every class, member and signature users can reach. Generated from the code by `scripts/api_surface.py` (`make api`); a change to the surface shows here as a diff, and necro fails while this file is stale. What each member does is in its docstring and [AUTHORING_GUIDE.md](AUTHORING_GUIDE.md).
"""


def strip_comment(line: str) -> str:
    """Drop a trailing # comment that sits outside string literals."""
    quote = ""
    for i, ch in enumerate(line):
        if quote:
            if ch == quote and line[i - 1] != "\\":
                quote = ""
        elif ch in "\"'":
            quote = ch
        elif ch == "#":
            return line[:i].rstrip()
    return line.rstrip()


def balance(text: str) -> int:
    """Open brackets minus closed ones, outside strings."""
    depth, quote = 0, ""
    for i, ch in enumerate(text):
        if quote:
            if ch == quote and text[i - 1] != "\\":
                quote = ""
        elif ch in "\"'":
            quote = ch
        elif ch in "([{":
            depth += 1
        elif ch in ")]}":
            depth -= 1
    return depth


def squash(parts: list[str]) -> str:
    """Join a wrapped declaration onto one line, as gdformat would unwrap it."""
    text = " ".join(p.strip() for p in parts)
    text = re.sub(r"\(\s+", "(", text)
    text = re.sub(r",?\s*\)", ")", text)
    return re.sub(r"\s+", " ", text)


def gd_statements(lines: list[str]):
    """Yield the file's top-level statements, each joined onto one line."""
    i = 0
    while i < len(lines):
        line = strip_comment(lines[i])
        i += 1
        if not line or line[0] in " \t":
            continue
        parts = [line]
        while balance(" ".join(parts)) > 0 and i < len(lines):
            parts.append(strip_comment(lines[i]))
            i += 1
        yield squash(parts)


def gd_class(path: Path, autoloads: dict[str, str]) -> tuple[str, list[str]] | None:
    """The class name and member lines of one script, or None when it isn't public."""
    rel = path.relative_to(ADDON).as_posix()
    name = autoloads.get(rel)
    base = ""
    members: list[str] = []
    pending: list[str] = []
    for stmt in gd_statements(path.read_text(encoding="utf-8").splitlines()):
        if m := re.match(r"class_name\s+(\w+)", stmt):
            name, pending = m[1], []
        elif m := re.match(r"extends\s+(\S+)", stmt):
            base, pending = m[1], []
        elif stmt.startswith("@") and not re.search(r"\b(var|func)\b", stmt):
            pending.append(stmt)
        else:
            entry = member(stmt, pending)
            pending = []
            if entry:
                members.append(entry)
    if name is None:
        return None
    return name, [f"extends {base}"] + members if base else members


def member(stmt: str, annotations: list[str]) -> str | None:
    """One public member line, or None for private and non-declaration statements."""
    stmt = re.sub(r"\s*:$", "", stmt)
    if m := re.match(
        r"((?:@\w+(?:\([^)]*\))?\s+)*)(static\s+)?(var|const)\s+(\w+)(.*)", stmt
    ):
        if m[4].startswith("_"):
            return None
        found = annotations + ([m[1].strip()] if m[1].strip() else [])
        # Editor layout (@export_group and friends) isn't part of the surface.
        notes = re.sub(
            r"@export_(group|subgroup|category)(\([^)]*\))?\s*", "", " ".join(found)
        )
        decl = f"{m[2] or ''}{m[3]} {m[4]}{m[5]}"
        if m[3] == "var" and "@export" not in notes:
            decl = re.sub(r"\s*=.*", "", decl)
        return f"{notes} {decl}".strip()
    if m := re.match(r"(static\s+)?func\s+(\w+)\s*(\(.*)", stmt):
        if m[2].startswith("_") and not (m[2] == "_init" and not m[3].startswith("()")):
            return None
        return f"{m[1] or ''}func {m[2]}{m[3]}"
    if stmt.startswith("signal "):
        return stmt
    if m := re.match(r"enum\s+(\w+)\s*\{(.*)\}", stmt):
        values = [v.strip().split("=")[0].strip() for v in m[2].split(",") if v.strip()]
        return f"enum {m[1]} {{ {', '.join(values)} }}"
    return None


def autoload_scripts() -> dict[str, str]:
    """Script path (relative to the addon) to autoload name, from plugin.gd."""
    text = (ADDON / "plugin.gd").read_text(encoding="utf-8")
    return {
        path: name
        for name, path in re.findall(
            r'add_autoload_singleton\("(\w+)",\s*"([^"]+)"\)', text
        )
    }


def rust_classes() -> list[tuple[str, list[str]]]:
    """Each GodotClass in the crate with its #[export] fields and #[func] methods."""
    classes: dict[str, list[str]] = {}
    for path in sorted(RUST_SRC.rglob("*.rs")):
        lines = path.read_text(encoding="utf-8").splitlines()
        base, current, impl, attrs = "", None, None, []
        for i, raw in enumerate(lines):
            line = raw.strip()
            if m := re.match(r"#\[class\(.*base\s*=\s*(\w+)", line):
                base = m[1]
            elif m := re.match(r"pub struct (\w+)", line):
                if base:
                    current = m[1]
                    classes[current] = [f"extends {base}"]
                base = ""
            elif current and line == "}":
                current = None
            elif m := re.match(r"impl (\w+) \{", line):
                impl = m[1] if m[1] in classes else None
            elif line.startswith("#["):
                attrs.append(line)
            elif (
                current
                and attrs
                and (m := re.match(r"(?:pub\s+)?(\w+):\s*(.+?),?$", line))
            ):
                if "#[export]" in attrs or "#[var]" in attrs:
                    classes[current].append(f"@export var {m[1]}: {m[2]}")
                attrs = []
            elif impl and "#[func]" in attrs and re.match(r"(pub\s+)?fn ", line):
                parts = [line]
                j = i
                while "{" not in parts[-1] and j + 1 < len(lines):
                    j += 1
                    parts.append(lines[j].strip())
                sig = squash(parts).split("{")[0].strip()
                sig = re.sub(r"^pub\s+", "", sig)
                sig = re.sub(r"\(&(mut )?self,?\s*", "(", sig)
                sig = re.sub(r"\b(?:crate|\w+)::(?:\w+::)*(\w+)", r"\1", sig)
                classes[impl].append(sig)
                attrs = []
            elif line and not line.startswith("///"):
                attrs = []
    return list(classes.items())


def render() -> str:
    autoloads = autoload_scripts()
    gd = []
    for path in sorted(ADDON.rglob("*.gd")):
        entry = gd_class(path, autoloads)
        if entry:
            gd.append((path.relative_to(ROOT).as_posix(), *entry))
    out = [PREAMBLE]
    out.append("## GDScript\n")
    for rel, name, members in sorted(gd, key=lambda e: e[1].lower()):
        out.append(
            f"### {name}\n\n`{rel}`\n\n```gdscript\n" + "\n".join(members) + "\n```\n"
        )
    out.append("## Rust (GDExtension)\n")
    for name, members in sorted(rust_classes(), key=lambda e: e[0].lower()):
        out.append(f"### {name}\n\n```rust\n" + "\n".join(members) + "\n```\n")
    return "\n".join(out)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--check", action="store_true", help="report staleness instead of writing"
    )
    check = parser.parse_args().check
    text = render()
    current = OUT.read_text(encoding="utf-8") if OUT.exists() else ""
    if check:
        if text != current:
            print(f"{OUT.relative_to(ROOT).as_posix()}:1: stale; run make api")
            return 1
        return 0
    if text != current:
        OUT.write_text(text, encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
