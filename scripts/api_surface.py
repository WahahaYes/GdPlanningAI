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

A class or member whose doc comment carries Godot's `@experimental` tag is
internal: it moves to the Internal section, which semver doesn't cover.

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


INTERNAL = """## Internal

Marked `@experimental` in their docs: the addon's own plumbing (the Rust bridge, the specs it serializes, the scheduler's job control). Reachable, but any release may change them; semver covers only the sections above.
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


def tagged(doc: list[str]) -> bool:
    """Whether a doc comment block carries the @experimental tag."""
    return any(re.match(r"(##|///)\s*@experimental\b", d.strip()) for d in doc)


def gd_statements(lines: list[str]):
    """Yield (statement, doc) for each top-level statement: the statement joined onto one
    line, and the ## block right above it."""
    i, doc = 0, []
    while i < len(lines):
        raw = lines[i]
        line = strip_comment(raw)
        i += 1
        if raw.startswith("##"):
            doc.append(raw)
            continue
        if not line or line[0] in " \t":
            if not raw.strip():
                doc = []
            continue
        parts = [line]
        while balance(" ".join(parts)) > 0 and i < len(lines):
            parts.append(strip_comment(lines[i]))
            i += 1
        yield squash(parts), doc
        doc = []


def class_doc(lines: list[str]) -> list[str]:
    """The script's class doc: its first block of ## lines."""
    block: list[str] = []
    for raw in lines:
        if raw.startswith("##"):
            block.append(raw)
        elif block:
            break
    return block


def gd_class(path: Path, autoloads: dict[str, str]):
    """(name, class is internal, [(member line, internal)]) for one script, or None when it
    defines no global class."""
    rel = path.relative_to(ADDON).as_posix()
    name = autoloads.get(rel)
    base = ""
    members: list[tuple[str, bool]] = []
    pending: list[str] = []
    lines = path.read_text(encoding="utf-8").splitlines()
    for stmt, doc in gd_statements(lines):
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
                members.append((entry, tagged(doc)))
    if name is None:
        return None
    head = [(f"extends {base}", False)] if base else []
    return name, tagged(class_doc(lines)), head + members


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


def rust_classes():
    """(name, class is internal, [(member line, internal)]) for each GodotClass in the crate,
    with its #[export] fields and #[func] methods."""
    classes: dict[str, list[tuple[str, bool]]] = {}
    internal: dict[str, bool] = {}
    for path in sorted(RUST_SRC.rglob("*.rs")):
        lines = path.read_text(encoding="utf-8").splitlines()
        base, current, impl, attrs, doc, class_tag = "", None, None, [], [], False
        for i, raw in enumerate(lines):
            line = raw.strip()
            if line.startswith("///"):
                doc.append(line)
                continue
            if m := re.match(r"#\[class\(.*base\s*=\s*(\w+)", line):
                base = m[1]
                class_tag = class_tag or tagged(doc)
            elif m := re.match(r"pub struct (\w+)", line):
                if base:
                    current = m[1]
                    classes[current] = [(f"extends {base}", False)]
                    internal[current] = class_tag
                base, class_tag = "", False
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
                    classes[current].append(
                        (f"@export var {m[1]}: {m[2]}", tagged(doc))
                    )
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
                classes[impl].append((sig, tagged(doc)))
                attrs = []
            elif line:
                attrs = []
            if not line.startswith("#[") and not re.match(r"#\[class", line):
                doc = []
    return [(name, internal[name], members) for name, members in classes.items()]


def block(name: str, where: str, lang: str, lines: list[str]) -> str:
    """One class's heading and code block."""
    loc = f"`{where}`\n\n" if where else ""
    return f"### {name}\n\n{loc}```{lang}\n" + "\n".join(lines) + "\n```\n"


def render() -> str:
    autoloads = autoload_scripts()
    classes = []  # (sort key, name, where, lang, class internal, members)
    for path in sorted(ADDON.rglob("*.gd")):
        entry = gd_class(path, autoloads)
        if entry:
            classes.append(
                (
                    entry[0].lower(),
                    entry[0],
                    path.relative_to(ROOT).as_posix(),
                    "gdscript",
                    *entry[1:],
                )
            )
    for name, is_internal, members in rust_classes():
        classes.append((name.lower(), name, "", "rust", is_internal, members))
    classes.sort()
    out = [PREAMBLE]
    for title, lang in (("GDScript", "gdscript"), ("Rust (GDExtension)", "rust")):
        out.append(f"## {title}\n")
        for _, name, where, kind, is_internal, members in classes:
            if kind == lang and not is_internal:
                out.append(
                    block(name, where, lang, [m for m, hidden in members if not hidden])
                )
    out.append(INTERNAL)
    for _, name, where, lang, is_internal, members in classes:
        if is_internal:
            out.append(block(name, where, lang, [m for m, _ in members]))
        elif any(hidden for _, hidden in members):
            out.append(
                block(
                    f"{name} (members)",
                    where,
                    lang,
                    [m for m, hidden in members if hidden],
                )
            )
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
