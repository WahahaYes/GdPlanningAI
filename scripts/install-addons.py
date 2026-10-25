"""Install Godot addons for CI by cloning the pins in addons.jsonc.

Local development uses GodotEnv (``make addons-install``); CI deliberately
avoids it so no .NET toolchain download is needed. Reads ``path`` and
``cache`` plus every ``remote`` entry from addons.jsonc, clones each pin
fresh, and copies its subfolder into the project addons directory.
"""

import json
import os
import shutil
import subprocess
import sys


def strip_jsonc(text: str) -> str:
    """Remove // and /* */ comments outside string literals."""
    out = []
    i = 0
    in_string = False
    while i < len(text):
        ch = text[i]
        if in_string:
            out.append(ch)
            if ch == "\\":
                out.append(text[i + 1 : i + 2])
                i += 1
            elif ch == '"':
                in_string = False
        elif ch == '"':
            in_string = True
            out.append(ch)
        elif ch == "/" and text[i + 1 : i + 2] == "/":
            while i < len(text) and text[i] != "\n":
                i += 1
            continue
        elif ch == "/" and text[i + 1 : i + 2] == "*":
            i += 2
            while i < len(text) - 1 and text[i : i + 2] != "*/":
                i += 1
            i += 1
        else:
            out.append(ch)
        i += 1
    return "".join(out)


def main(project_dir: str) -> None:
    """Clone every remote addon pin into the project addons directory."""
    config_path = os.path.join(project_dir, "addons.jsonc")
    with open(config_path, encoding="utf-8") as f:
        config = json.loads(strip_jsonc(f.read()))
    path = config.get("path", "addons")
    cache = config.get("cache", ".addons")
    for name, spec in config["addons"].items():
        source = spec.get("source", "remote")
        if source != "remote":
            sys.exit(f"install-addons: CI supports remote addons only, got {source}")
        url = spec["url"]
        checkout = spec.get("checkout", "main")
        subfolder = spec.get("subfolder", "/").lstrip("/")
        clone_dir = os.path.join(project_dir, cache, name)
        shutil.rmtree(clone_dir, ignore_errors=True)
        subprocess.run(
            ["git", "clone", "--depth", "1", "--branch", checkout, url, clone_dir],
            check=True,
        )
        target = os.path.join(project_dir, path, name)
        shutil.rmtree(target, ignore_errors=True)
        shutil.copytree(os.path.join(clone_dir, subfolder), target)
        print(f"install-addons: {name}@{checkout} -> {target}")


if __name__ == "__main__":
    if len(sys.argv) > 1:
        main(sys.argv[1])
    else:
        main(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
