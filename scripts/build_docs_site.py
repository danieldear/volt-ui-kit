#!/usr/bin/env python3
"""Stage only the reviewed static site, never the repository or work logs."""
import argparse
from pathlib import Path
import shutil

ROOT = Path(__file__).resolve().parents[1]
FILES = (
    "index.html", ".nojekyll", "assets/site.css", "assets/favicon.svg",
    "images/dashboard-dark.png", "images/theme-editor-light.png",
)


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
