#!/usr/bin/env python3
"""Stage only the reviewed static site, never the repository or work logs."""
import argparse
import json
from pathlib import Path
import re
import shutil

ROOT = Path(__file__).resolve().parents[1]
BASE_FILES = (
    "index.html", ".nojekyll", "assets/site.css", "assets/favicon.svg",
    "images/dashboard-dark.png", "images/theme-editor-light.png",
    "assets/fonts/bricolage-grotesque.ttf", "assets/fonts/OFL.txt",
    "assets/reference.css",
)


def generated_files(root: Path) -> tuple[str, ...]:
    """Only publish reviewed HTML, preview PNGs and the Catalog data export."""
    entries = json.loads((root / "docs/site-manifest.json").read_text())
    allowed = r"(?:(?:reference|guides)/[a-z0-9-]+\.html|images/components/[a-z0-9-]+\.png|assets/catalog-data\.json)"
    if not isinstance(entries, list) or any(
        not isinstance(name, str) or not re.fullmatch(allowed, name) for name in entries
    ) or len(entries) != len(set(entries)):
        raise ValueError("invalid generated documentation manifest")
    return tuple(entries)


FILES = BASE_FILES + generated_files(ROOT)


def build(output: Path) -> None:
    output = output.absolute()
    # No cleanup and no user-selected deletion path: staging must be brand new.
    if output.exists() or output.is_symlink():
        raise ValueError("site output already exists; use a new staging directory")
    sources = [ROOT / "docs" / file for file in FILES]
    for source in sources:
        if source.is_symlink() or not source.is_file():
            raise ValueError(f"missing or linked site asset: {source.name}")
        if not source.resolve().is_relative_to((ROOT / "docs").resolve()):
            raise ValueError("asset escapes docs directory")
    output.mkdir(parents=True)
    for name, source in zip(FILES, sources):
        target = output / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
    print(f"Staged {len(FILES)} static site files in {output}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    build(parser.parse_args().output)
