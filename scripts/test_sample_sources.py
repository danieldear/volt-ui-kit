"""Copied-program lock preparation and the pinned real-browser CI runtime."""
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import check_sample_sources

ROOT = Path(__file__).resolve().parents[1]


class CopiedSampleTests(unittest.TestCase):
    def test_fixture_is_prepared_offline_before_the_locked_compile(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            lock = b'version = 4\n# repository lock must stay unchanged\n'
            (root / 'Cargo.lock').write_bytes(lock)
            source = 'fn main() { println!("UTC\u22128"); }\n'
            commands = []
            fixture_paths = []

            def cargo(command, **options):
                fixture = Path(command[command.index('--manifest-path') + 1]).parent
                fixture_paths.append(fixture)
                self.assertTrue(options['check'])
                self.assertEqual(options['env']['CARGO_TARGET_DIR'], str((root / 'cache').resolve()))
                self.assertEqual((fixture / 'button-states.rs').read_text(encoding='utf-8'), source)
                if command[1] == 'update':
                    self.assertEqual((fixture / 'Cargo.lock').read_bytes(), lock)
                    self.assertIn('--offline', command)
                    self.assertIn('--workspace', command)
                    self.assertNotIn('--locked', command)
                    self.assertEqual(options['stdout'], subprocess.DEVNULL)
                    (fixture / 'Cargo.lock').write_bytes(lock + b'# prepared fixture graph\n')
                else:
                    self.assertEqual(commands, ['update'])
                    self.assertEqual(command[1], 'check')
                    self.assertIn('--offline', command)
                    self.assertIn('--locked', command)
                    self.assertIn('--all-targets', command)
                    self.assertIn(b'prepared fixture graph', (fixture / 'Cargo.lock').read_bytes())
                commands.append(command[1])

            with patch.object(check_sample_sources, 'ROOT', root), \
                    patch.dict(os.environ, {'CARGO_TARGET_DIR': str(root / 'cache')}), \
                    patch.object(check_sample_sources.subprocess, 'check_output',
                                 side_effect=['Usage: samples\n  button-states\n', source]) as output, \
                    patch.object(check_sample_sources.subprocess, 'run', side_effect=cargo), \
                    patch('builtins.print'):
                check_sample_sources.check(root / 'samples')
            self.assertEqual(commands, ['update', 'check'])
            self.assertEqual((root / 'Cargo.lock').read_bytes(), lock)
            self.assertTrue(all(not p.exists() for p in fixture_paths))
            for call in output.call_args_list:
                self.assertEqual(call.kwargs['encoding'], 'utf-8')

    def test_failed_preparation_does_not_compile_and_cleans_up(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'Cargo.lock').write_text('version = 4\n', encoding='utf-8')
            paths = []

            def fail(command, **_options):
                paths.append(Path(command[command.index('--manifest-path') + 1]).parent)
                self.assertEqual(command[1], 'update')
                raise subprocess.CalledProcessError(101, command)

            with patch.object(check_sample_sources, 'ROOT', root), \
                    patch.object(check_sample_sources.subprocess, 'check_output',
                                 side_effect=['  table\n', 'fn main() {}\n']), \
                    patch.object(check_sample_sources.subprocess, 'run', side_effect=fail) as run:
                with self.assertRaises(subprocess.CalledProcessError):
                    check_sample_sources.check(root / 'samples')
            self.assertEqual(run.call_count, 1)
            self.assertFalse(paths[0].exists())

    def test_browser_ci_pins_a_matching_image_and_retains_all_real_tests(self):
        workflow = (ROOT / '.github/workflows/web-prototype.yml').read_text()
        image = re.search(r'PLAYWRIGHT_IMAGE: mcr\.microsoft\.com/playwright:v([\d.]+)-noble@sha256:([0-9a-f]{64})', workflow)
        self.assertIsNotNone(image)
        self.assertIn('playwright@' + image.group(1), workflow)
        self.assertIn('runs-on: ubuntu-24.04', workflow)
        self.assertIn('--env PLAYWRIGHT_BROWSERS_PATH=/ms-playwright', workflow)
        self.assertIn('docker run --rm --interactive', workflow)
        self.assertIn('--user "$(id -u):$(id -g)"', workflow)
        self.assertNotIn('    container:', workflow)
        self.assertIn('cargo test --locked --manifest-path crates/volt-ui-kit-web/Cargo.toml', workflow)
        self.assertNotIn('apt-get', workflow)
        self.assertNotIn('install --with-deps', workflow)
        for script in ('test_web.cjs', 'test_catalog_web.cjs', 'test_web_input.cjs'):
            self.assertIn('node scripts/' + script, workflow)


if __name__ == '__main__':
    unittest.main()
