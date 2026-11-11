#!/usr/bin/env python3
"""Compile the exact copied Catalog programs; no windows, installs or publishing.

Build `cargo build --locked --features native --example samples` first. Uses
CARGO_TARGET_DIR if set and seeds the temporary fixture with the repository's
lockfile (including its currently locked yanked transitive dependencies).
"""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def check(runner=None):
    target = Path(os.environ.get("CARGO_TARGET_DIR", str(ROOT / "target"))).resolve()
    runner = Path(runner) if runner else target / "debug/examples" / ("samples.exe" if os.name == "nt" else "samples")
    listing = subprocess.check_output([str(runner), "--list"], text=True, encoding="utf-8")
    names = [line.strip() for line in listing.splitlines() if re.fullmatch(r"  [a-z]+(?:-[a-z]+)*", line)]
    if not names or len(names) != len(set(names)):
        raise ValueError("runner must list distinct safe recipe names")
    with tempfile.TemporaryDirectory(prefix="volt-copied-samples-") as directory:
        fixture = Path(directory)
        manifest = ["[package]", 'name = "volt-copied-catalog-recipes"', 'version = "0.0.0"',
                    'edition = "2021"', "publish = false", "[dependencies]",
                    f'volt-ui-kit = {{ path = {json.dumps(ROOT.as_posix())}, features = ["native"] }}']
        for name in names:
            source = subprocess.check_output([str(runner), "--source", name], text=True, encoding="utf-8")
            if "catalog-only" in source or "#[cfg(" in source:
                raise ValueError(f"{name}: copied code still contains module-only guards")
            (fixture / f"{name}.rs").write_text(source, encoding="utf-8")
            manifest.extend(["[[example]]", f"name = {json.dumps(name)}", f"path = {json.dumps(name + '.rs')}"])
        (fixture / "Cargo.toml").write_text("\n".join(manifest) + "\n", encoding="utf-8")
        shutil.copyfile(ROOT / "Cargo.lock", fixture / "Cargo.lock")
        env = dict(os.environ, CARGO_TARGET_DIR=str(target))
        subprocess.run(
            [
                "cargo",
                "check",
                "--locked",
                "--offline",
                "--all-targets",
                "--manifest-path",
                str(fixture / "Cargo.toml"),
            ],
            check=True,
            env=env,
        )
    print(f"Checked {len(names)} exact copied Catalog programs against this toolkit; no windows opened.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runner", type=Path, help="explicit compiled samples executable")
    args = parser.parse_args()
    check(args.runner)
