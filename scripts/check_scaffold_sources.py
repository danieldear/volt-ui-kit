#!/usr/bin/env python3
"""Generate and compile every desktop starter without launching or installing apps."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
TEMPLATES = ('minimal', 'navigation', 'mail', 'settings', 'agent')


def check(runner=None):
    target = Path(os.environ.get('CARGO_TARGET_DIR', ROOT / 'target')).resolve()
    runner = Path(runner) if runner else target / 'debug' / ('cargo-volt.exe' if os.name == 'nt' else 'cargo-volt')
    env = dict(os.environ, CARGO_TARGET_DIR=str(target))
    with tempfile.TemporaryDirectory(prefix='volt-starters-') as directory:
        fixture = Path(directory)
        manifest = ['[package]', 'name = "volt-starter-check"', 'version = "0.0.0"',
                    'edition = "2021"', '[workspace]', '[dependencies]',
                    'volt-ui-kit = {path = ' + json.dumps(str(ROOT)) + ', features = ["native-gpu", "native-desktop"]}']
        for kind in TEMPLATES:
            for desktop in (False, True):
                name = kind + ('-desktop' if desktop else '-app')
                project = fixture / name
                command = [str(runner), 'new', str(project), '--kit-path', str(ROOT), '--template', kind]
                if desktop:
                    command.append('--desktop')
                subprocess.run(command, check=True, env=env)
                app_id = 'dev.volt.' + name.replace('-', '_')
                source = (project / 'src/main.rs').read_text(encoding='utf-8')
                launcher = (project / f'packaging/linux/{app_id}.desktop').read_text(encoding='utf-8')
                if f'.app_id("{app_id}")' not in source or f'StartupWMClass={app_id}\n' not in launcher:
                    raise ValueError(f'{name}: app identity does not match the launcher')
                if f'Exec={name}\n' not in launcher or 'Terminal=false\n' not in launcher:
                    raise ValueError(f'{name}: invalid application launcher')
                icon = project / f'packaging/linux/{app_id}.svg'
                if f'Icon={app_id}\n' not in launcher or '<svg' not in icon.read_text(encoding='utf-8'):
                    raise ValueError(f'{name}: missing matching application icon')
                manifest += ['[[bin]]', 'name = ' + json.dumps(name), 'path = ' + json.dumps(str(project / 'src/main.rs'))]
        (fixture / 'Cargo.toml').write_text('\n'.join(manifest) + '\n', encoding='utf-8')
        shutil.copyfile(ROOT / 'Cargo.lock', fixture / 'Cargo.lock')
        subprocess.run(['cargo', 'update', '--offline', '--workspace', '--manifest-path', str(fixture / 'Cargo.toml')], check=True, env=env, stdout=subprocess.DEVNULL)
        subprocess.run(['cargo', 'check', '--locked', '--offline', '--all-targets', '--manifest-path', str(fixture / 'Cargo.toml')], check=True, env=env)
    print('Checked all 10 desktop starters and matching Linux launchers; no windows opened or launchers installed.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--runner', type=Path)
    check(parser.parse_args().runner)
