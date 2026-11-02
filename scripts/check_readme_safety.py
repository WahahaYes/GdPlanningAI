"""README paste-safety for the asset store and asset library listings.

The README is pasted by hand into the Asset Store and Asset Library
description pages, so it has to stand alone off GitHub. This check fails
when README.md:

- links a repo page or embeds a repo file by relative path (markdown
  links/images, HTML src/href): pages must use
  https://github.com/WahahaYes/GdPlanningAI/blob/main/<path>, media
  https://raw.githubusercontent.com/WahahaYes/GdPlanningAI/refs/heads/main/<path>;
  in-page #anchors are fine;
- points an absolute repo URL at a path that does not exist in the tree;
- is longer than 20000 characters.

Usage:
  uv run scripts/check_readme_safety.py [README...]
  uv run scripts/check_readme_safety.py --explain <code|all>

Exit code 0 clean, 1 on any violation, 2 on a config error.
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CANONICAL = ROOT / "README.md"
ADDON_COPY = ROOT / "addons/GdPlanningAI/README.md"

REPO = "WahahaYes/GdPlanningAI"
BLOB_PREFIX = f"https://github.com/{REPO}/blob/main/"
RAW_PREFIX = f"https://raw.githubusercontent.com/{REPO}/refs/heads/main/"
MAX_CHARS = 20000

MEDIA_EXTS = {
    ".png",
    ".jpg",
    ".jpeg",
    ".webp",
    ".avif",
    ".svg",
    ".gif",
    ".mp4",
    ".webm",
    ".mp3",
    ".wav",
    ".ogg",
    ".ico",
    ".glb",
    ".gltf",
    ".ttf",
    ".woff",
    ".woff2",
}

RE_MD_LINK = re.compile(r"!?\[[^\]]*\]\(\s*(<[^>]+>|[^)\s]+)(?:\s+[^)]*)?\)")
RE_HTML_ATTR = re.compile(r"""(?:src|href)\s*=\s*("[^"]*"|'[^']*')""", re.IGNORECASE)

RULES = {
    "readme-relative-link": {
        "summary": "relative repo link {dest} breaks off GitHub",
        "why": "the README is pasted into the Asset Store and Library descriptions, where relative links break",
        "help": "use {expected} for {kind} ({BLOB_PREFIX}<path> for pages, {RAW_PREFIX}<path> for media)",
        "explain": (
            "## What the rule says\n\n"
            "The README is pasted by hand into the Asset Store and Asset "
            "Library description pages, so every repo link must stand alone "
            "off GitHub. Markdown links/images and HTML src/href attributes "
            "may not use relative paths.\n\n"
            "## Why\n\n"
            "Relative paths resolve on github.com but break in the store and "
            "library listings, which have no repo tree to resolve against.\n\n"
            "## Bad / Good\n\n"
            "Bad: `[algorithm](docs/ALGORITHM.md)` or "
            "`![banner](media/gdpai_banner.png)`.\n\n"
            "Good (page): `[algorithm](https://github.com/WahahaYes/GdPlanningAI/blob/main/docs/ALGORITHM.md)`.\n\n"
            'Good (media): `<img src="https://raw.githubusercontent.com/WahahaYes/GdPlanningAI/refs/heads/main/media/gdpai_banner.png">`.\n\n'
            "In-page anchors (`#section`) and links outside the repo "
            "(godotengine.org, youtube.com, shields.io, codecov.io, ...) are fine.\n\n"
            "## When an exception is fair\n\n"
            "Never for store-bound text: keep the README paste-safe and put "
            "repo-relative shortcuts in docs/ or notes/ instead."
        ),
    },
    "readme-missing-file": {
        "summary": "repo URL points at {dest}, which is not in the tree",
        "why": "an absolute repo URL that names no file breaks the same listings the relative-link rule protects",
        "help": "point it at a path that exists under the repo root, or drop the link",
        "explain": (
            "## What the rule says\n\n"
            "Absolute repo URLs (github.com blob links and "
            "raw.githubusercontent.com links) must name a path that exists "
            "in the checkout.\n\n"
            "## Why\n\n"
            "A pasted README cannot be clicked through to check; a typo in "
            "the path ships a dead link to both catalogs.\n\n"
            "## Bad / Good\n\n"
            "Bad: `.../blob/main/docs/ALGORTHM.md` (typo, no such file).\n\n"
            "Good: `.../blob/main/docs/ALGORITHM.md`.\n\n"
            "Non-file repo pages (releases, issues, pulls, the asset page) "
            "are not file paths and are not checked.\n\n"
            "## When an exception is fair\n\n"
            "When the file lands in a later commit of the same change: the "
            "hook then passes once both are staged."
        ),
    },
    "readme-too-long": {
        "summary": "README is {actual} characters (max {MAX_CHARS})",
        "why": "the store and library description fields reject over-long pastes",
        "help": "cut the README to {MAX_CHARS} characters; move reasoning to docs/ or notes/",
        "explain": (
            "## What the rule says\n\n"
            "README.md must stay at or under 20000 characters.\n\n"
            "## Why\n\n"
            "The text is pasted by hand into two catalog description fields; "
            "over-long input gets truncated or rejected there.\n\n"
            "## Bad / Good\n\n"
            "Bad: a README that keeps growing past the limit.\n\n"
            "Good: a tight README with detail in docs/ behind absolute links.\n\n"
            "## When an exception is fair\n\n"
            "Raise the limit only when a catalog raises its field, and update "
            "this rule and the bead together."
        ),
    },
}


def explain(code: str) -> int:
    if code == "all":
        for name, rule in RULES.items():
            print(f"### {name}\n\n{rule['explain']}\n")
        return 0
    rule = RULES.get(code)
    if rule is None:
        print(
            f"check_readme_safety: unknown code {code!r} "
            f"(known: {', '.join(sorted(RULES))}, all)",
            file=sys.stderr,
        )
        return 2
    print(f"### {code}\n\n{rule['explain']}")
    return 0


def is_media(path: str) -> bool:
    lower = path.lower().split("?", 1)[0].split("#", 1)[0]
    suffix = Path(lower).suffix
    return suffix in MEDIA_EXTS


def strip_anchors(dest: str) -> str:
    return dest.strip().strip("<>").strip()


def check_text(text: str, display: Path) -> list[tuple]:
    """Return (path, lineno, col, code, summary, why, help) hits."""
    hits: list[tuple] = []
    lines = text.splitlines()

    def emit(lineno: int, col: int, code: str, summary: str, help_text: str):
        hits.append(
            (display, lineno, col, code, summary, RULES[code]["why"], help_text)
        )

    for i, raw in enumerate(lines):
        lineno = i + 1
        for m in RE_MD_LINK.finditer(raw):
            dest = strip_anchors(m.group(1))
            col = m.start(1) + 1
            hit = check_dest(dest, is_image=m.group(0).startswith("!"))
            if hit is not None:
                code, summary, help_text = hit
                emit(lineno, col, code, summary, help_text)
        for m in RE_HTML_ATTR.finditer(raw):
            dest = m.group(1)[1:-1].strip()
            col = m.start(1) + 1
            hit = check_dest(dest, is_image=True)
            if hit is not None:
                code, summary, help_text = hit
                emit(lineno, col, code, summary, help_text)

    if len(text) > MAX_CHARS:
        summary = RULES["readme-too-long"]["summary"].format(
            actual=len(text), MAX_CHARS=MAX_CHARS
        )
        help_text = RULES["readme-too-long"]["help"].format(MAX_CHARS=MAX_CHARS)
        emit(1, 1, "readme-too-long", summary, help_text)

    # Absolute repo URLs pointing outside the tree (checked after the
    # per-line pass so relative-link hits keep line order).
    for i, raw in enumerate(lines):
        lineno = i + 1
        for m in RE_MD_LINK.finditer(raw):
            dest = strip_anchors(m.group(1))
            missing = check_exists(dest)
            if missing is not None:
                code, summary, help_text = missing
                emit(lineno, m.start(1) + 1, code, summary, help_text)
        for m in RE_HTML_ATTR.finditer(raw):
            dest = m.group(1)[1:-1].strip()
            missing = check_exists(dest)
            if missing is not None:
                code, summary, help_text = missing
                emit(lineno, m.start(1) + 1, code, summary, help_text)

    return hits


def check_dest(dest: str, is_image: bool):
    """Rule readme-relative-link, or None when the destination is fine."""
    if not dest or dest.startswith("#"):
        return None
    lowered = dest.lower()
    if lowered.startswith(("http://", "https://", "mailto:", "data:", "ftp://", "//")):
        return None
    kind = "media" if (is_image or is_media(dest)) else "page"
    if kind == "media":
        expected = f"{RAW_PREFIX}{dest.lstrip('./')}"
    else:
        expected = f"{BLOB_PREFIX}{dest.lstrip('./')}"
    summary = RULES["readme-relative-link"]["summary"].format(dest=dest)
    help_text = RULES["readme-relative-link"]["help"].format(
        expected=expected, kind=kind, BLOB_PREFIX=BLOB_PREFIX, RAW_PREFIX=RAW_PREFIX
    )
    return ("readme-relative-link", summary, help_text)


def check_exists(dest: str):
    """Rule readme-missing-file for absolute repo URLs, else None."""
    path = None
    if dest.startswith(BLOB_PREFIX):
        path = dest[len(BLOB_PREFIX) :]
    elif dest.startswith(RAW_PREFIX):
        path = dest[len(RAW_PREFIX) :]
    else:
        return None
    path = path.split("#", 1)[0].split("?", 1)[0].strip("/")
    if not path:
        return None
    if (ROOT / path).exists():
        return None
    summary = RULES["readme-missing-file"]["summary"].format(dest=dest)
    return ("readme-missing-file", summary, RULES["readme-missing-file"]["help"])


def report(hits: list[tuple]) -> int:
    """Print hits and return an exit code.

    Hits go to stdout, one necro hit-line each
    (``path:line:col: [code] summary``); the why/help/explain guidance and
    the count summary go to stderr so necro never tries to parse them.
    """
    if not hits:
        print("check_readme_safety: README is paste-safe.", file=sys.stderr)
        return 0
    shown_why: set[str] = set()
    for display, lineno, col, code, summary, why, help_text in sorted(
        hits, key=lambda h: (str(h[0]), h[1], h[2])
    ):
        print(f"{display}:{lineno}:{col}: [{code}] {summary}")
        if code not in shown_why:
            print(f"  why: {why}", file=sys.stderr)
            print(f"  help: {help_text}", file=sys.stderr)
            print(
                f"  explain: uv run scripts/check_readme_safety.py --explain {code}",
                file=sys.stderr,
            )
            shown_why.add(code)
        else:
            print(f"  help: {help_text}", file=sys.stderr)
    print(f"check_readme_safety: {len(hits)} violation(s) found.", file=sys.stderr)
    return 1


def main(argv: list[str]) -> int:
    args = [a for a in argv if a != "--"]
    if "--explain" in args:
        idx = args.index("--explain")
        try:
            code = args[idx + 1]
        except IndexError:
            print(
                "check_readme_safety: --explain needs a code "
                f"({', '.join(sorted(RULES))}, all)",
                file=sys.stderr,
            )
            return 2
        return explain(code)
    targets = [a for a in args if not a.startswith("-")]
    files: list[Path] = []
    if targets:
        for t in targets:
            p = Path(t)
            if not p.is_absolute():
                p = ROOT / p
            # The addon copy must match the root (make check-docs); the
            # canonical text is what ships, so check it once.
            if p.resolve() == ADDON_COPY.resolve():
                p = CANONICAL
            if p not in files:
                files.append(p)
    else:
        files = [CANONICAL]
    all_hits: list[tuple] = []
    for path in files:
        try:
            text = path.read_text(encoding="utf-8")
        except OSError as exc:
            print(f"check_readme_safety: could not read {path}: {exc}", file=sys.stderr)
            return 2
        try:
            display = path.relative_to(ROOT)
        except ValueError:
            display = path
        all_hits.extend(check_text(text, display))
    return report(all_hits)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
