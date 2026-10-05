"""Collect dependency license files for one release target, without choosing licenses."""

import json
from pathlib import Path
import subprocess
import sys


def collect(package):
    root = Path(package["manifest_path"]).parent
    names = ("license", "licence", "copying", "copyright", "notice", "unlicense")
    files = {
        path for path in root.rglob("*")
        if path.is_file() and path.name.lower().startswith(names)
    }
    if package.get("license_file"):
        files.add(root / package["license_file"])
    if not files:
        raise ValueError(f"No license files for {package['name']} {package['version']}")
    return {
        "name": package["name"],
        "version": package["version"],
        "license": package["license"],
        "files": {str(path.relative_to(root)): path.read_text(encoding="utf-8")
                  for path in sorted(files)},
    }


def main():
    target, output = sys.argv[1:]
    metadata = json.loads(subprocess.check_output([
        "cargo", "metadata", "--locked", "--format-version", "1",
        "--filter-platform", target,
    ]))
    resolved = {node["id"] for node in metadata["resolve"]["nodes"]}
    packages = [p for p in metadata["packages"]
                if p["id"] in resolved and p["id"] != metadata["resolve"]["root"]]
    notices = [collect(p) for p in sorted(packages, key=lambda p: (p["name"], p["version"]))]
    Path(output).write_text(json.dumps(notices, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
