#!/usr/bin/env python3
"""Reproducibly vendor this canonical toolkit into a Volt checkout."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import sys

ROOT = Path(__file__).resolve().parents[1]

def sources():
    files = [ROOT / n for n in ("Cargo.toml", "README.md", "LICENSE", "CHANGELOG.md", "ROADMAP.md")]
    files += [p for d in ("src", "examples", "tests", "docs") for p in (ROOT / d).rglob("*") if p.is_file()]
    return {str(p.relative_to(ROOT)): p for p in sorted(files)}

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--volt", type=Path, required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    volt = args.volt.resolve()
    if not (volt / "volt-renderer/Cargo.toml").is_file():
        parser.error("destination is not a Volt workspace")
    destination = volt / "vendor/volt-ui-kit"
    files = sources()
    manifest = {"project": "volt-ui-kit", "version": "0.4.0", "files": {
        name: hashlib.sha256(path.read_bytes()).hexdigest() for name, path in files.items()}}
    marker = destination / "SOURCE.json"
    if destination.is_symlink() or marker.is_symlink():
        parser.error("refusing to follow a vendor directory or manifest symlink")
    if destination.exists() and any(destination.iterdir()):
        try:
            previous = json.loads(marker.read_text())
            if previous.get("project") != "volt-ui-kit": raise ValueError("wrong project")
        except (OSError, ValueError):
            parser.error("refusing to overwrite an unrecognized nonempty directory")
    inventory = {str(p.relative_to(destination)) for p in destination.rglob("*") if p.is_file()} if destination.exists() else set()
    expected = set(files) | {"SOURCE.json"}
    if args.check:
        matches = (inventory == expected and marker.is_file()
                   and json.loads(marker.read_text()) == manifest
                   and all((destination / n).read_bytes() == p.read_bytes() for n,p in files.items()))
        if not matches: print("Toolkit snapshot differs; sync the canonical source first.", file=sys.stderr); return 1
        print("Toolkit snapshot matches canonical source."); return 0
    # Never remove files not listed by the previous generated snapshot.
    for name in sorted(inventory - expected):
        if name not in previous.get("files", {}):
            parser.error(f"unexpected file in vendor directory: {name}; refusing to remove it")
        path = destination / name
        if path.is_symlink(): parser.error("refusing to remove vendor symlink")
        path.unlink()
    for name,path in files.items():
        target = destination / name
        if target.is_symlink() or any(p.is_symlink() for p in target.parents if p != volt.parent):
            parser.error("refusing to follow symlinks in vendor destination")
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, target)
    marker.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    print(f"Vendored {len(files)} toolkit files into {destination}"); return 0

if __name__ == "__main__":
    sys.exit(main())
