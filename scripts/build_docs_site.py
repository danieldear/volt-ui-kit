#!/usr/bin/env python3
"""Stage only the reviewed static site, never the repository or work logs."""
import argparse
import json
from pathlib import Path
import re
import shutil

ROOT = Path(__file__).resolve().parents[1]
# Only source-backed document assets and social metadata images are published.
BASE_FILES = (
    "index.html", ".nojekyll", "assets/site.css", "assets/favicon.svg",
    "images/og-card.jpg", "assets/fonts/bricolage-grotesque.woff2", "assets/fonts/OFL.txt",
    "assets/reference.css", "live/index.html", "live/live.css", "live/live.js", "live/OFL.txt",
    "live/catalog-previews.js", "live/JetBrainsMono-OFL.txt",
)


def generated_files(root: Path) -> tuple[str, ...]:
    """Only publish reviewed HTML and the Catalog data export."""
    entries = json.loads((root / "docs/site-manifest.json").read_text())
    allowed = r"(?:(?:reference|guides)/[a-z0-9-]+\.html|assets/catalog-data\.json)"
    if not isinstance(entries, list) or any(
        not isinstance(name, str) or not re.fullmatch(allowed, name) for name in entries
    ) or len(entries) != len(set(entries)):
        raise ValueError("invalid generated documentation manifest")
    return tuple(entries)


FILES = BASE_FILES + generated_files(ROOT)


def build(output: Path, web_assets: Path | None = None) -> None:
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
    # Generated WASM is never accepted through the documentation manifest.
    # Explicitly stage exactly the two reviewed wasm-bindgen outputs plus the
    # canonical adapter. No arbitrary JS/directory copying from a build folder.
    runtime = ("volt_web_catalog.js", "volt_web_catalog_bg.wasm")
    adapter = ROOT / "crates/volt-ui-kit-web/web/volt-host.js"
    editing = ROOT / "crates/volt-ui-kit-web/web/text-input.js"
    if editing.is_symlink() or not editing.is_file():
        raise ValueError("missing or linked canonical text adapter")
    if adapter.is_symlink() or not adapter.is_file():
        raise ValueError("missing or linked canonical browser adapter")
    if web_assets is not None:
        if web_assets.is_symlink() or not web_assets.is_dir():
            raise ValueError("web assets must be a real build directory")
        for name in runtime:
            asset = web_assets / name
            if asset.is_symlink() or not asset.is_file() or not asset.resolve().is_relative_to(web_assets.resolve()):
                raise ValueError(f"missing or linked web runtime: {name}")
    output.mkdir(parents=True)
    for name, source in zip(FILES, sources):
        target = output / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
    shutil.copyfile(adapter, output / "live/volt-host.js")
    shutil.copyfile(editing, output / "live/text-input.js")
    if web_assets is not None:
        (output / "live/pkg").mkdir()
        for name in runtime:
            shutil.copyfile(web_assets / name, output / "live/pkg" / name)
    print(f"Staged {len(FILES)} static site files in {output}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--web-assets", type=Path, help="explicit wasm-bindgen output directory")
    args = parser.parse_args()
    build(args.output, args.web_assets)
