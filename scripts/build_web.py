#!/usr/bin/env python3
"""Build the opt-in live Rust/WASM docs demo. Does not install tools or publish.

Requires wasm32-unknown-unknown and wasm-bindgen-cli 0.2.129 on PATH (or
WASM_BINDGEN). The generated module stays in ignored docs/live/pkg/.
"""
import argparse
import os
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
VERSION = "0.2.129"


def build():
    output = ROOT / "docs/live/pkg"
    adapter = ROOT / "docs/live/volt-host.js"
    for path in (ROOT / "crates/volt-ui-kit-web/web/volt-host.js", ROOT / "crates/volt-ui-kit-web/web/text-input.js", ROOT / "docs", output.parent, output, adapter,
                 output / "volt_web_catalog.js", output / "volt_web_catalog_bg.wasm", ROOT / "docs/live/text-input.js"):
        if path.is_symlink():
            raise ValueError("web source/output must not traverse symlinks")
    local_tool = ROOT / "target/web-tools/wasm-bindgen"
    tool = os.environ.get("WASM_BINDGEN") or shutil.which("wasm-bindgen")
    if not tool and local_tool.is_file() and not local_tool.is_symlink():
        tool = str(local_tool)
    if not tool:
        raise ValueError(f"Install wasm-bindgen-cli {VERSION}, or set WASM_BINDGEN to its executable")
    if subprocess.check_output([tool, "--version"], text=True).strip() != f"wasm-bindgen {VERSION}":
        raise ValueError(f"wasm-bindgen-cli must be {VERSION} to match the locked Rust dependency")
    target = ROOT / "target/web"
    env = dict(os.environ, CARGO_TARGET_DIR=str(target))
    subprocess.run(["cargo", "build", "--locked", "--release", "--target", "wasm32-unknown-unknown",
                    "--manifest-path", str(ROOT / "examples/web/Cargo.toml")], check=True, env=env)
    subprocess.run([tool, str(target / "wasm32-unknown-unknown/release/volt_web_catalog.wasm"),
                    "--target", "web", "--no-typescript", "--out-dir", str(output)], check=True)
    shutil.copyfile(ROOT / "crates/volt-ui-kit-web/web/volt-host.js", ROOT / "docs/live/volt-host.js")
    shutil.copyfile(ROOT / "crates/volt-ui-kit-web/web/text-input.js", ROOT / "docs/live/text-input.js")
    size = (output / "volt_web_catalog_bg.wasm").stat().st_size
    print(f"Built live Rust demo ({size:,} WASM bytes). Serve docs/ over localhost or HTTPS.")


if __name__ == "__main__":
    argparse.ArgumentParser(description=__doc__).parse_args()
    try:
        build()
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        raise SystemExit(f"Web build failed: {error}")
