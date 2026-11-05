"""Exercise launcher control flow without GPU, GUI, permissions or network access."""
import json
import os
import re
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / 'scripts/run_catalog.sh'

@unittest.skipUnless(os.name == "posix", "POSIX Catalog launcher")
class CatalogLauncherTests(unittest.TestCase):
    def run_launcher(self, args=(), fail=False):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            receipt = root / 'calls'
            exe = root / 'Catalog with spaces'
            exe.write_text('#!/bin/sh\nprintf executed >> "$CALLS"\n')
            exe.chmod(0o700)
            artifact = json.dumps({'reason': 'compiler-artifact', 'target': {
                'name': 'catalog', 'kind': ['example']}, 'executable': str(exe)})
            cargo = root / 'cargo'
            cargo.write_text('#!/bin/sh\nprintf "%s\\n" "$*" >> "$CALLS"\n' +
                             ('exit 9\n' if fail else "printf '%s\\n' '" + artifact + "'\n"))
            cargo.chmod(0o700)
            uname = root / 'uname'
            uname.write_text('#!/bin/sh\nprintf Linux\n')
            uname.chmod(0o700)
            env = dict(os.environ, PATH=str(root) + os.pathsep + os.environ['PATH'],
                       CALLS=str(receipt), TMPDIR=tmp)
            result = subprocess.run(['/bin/sh', str(SCRIPT), *args], env=env,
                                    capture_output=True, text=True)
            calls = receipt.read_text() if receipt.exists() else ''
            leftovers = list(root.glob('volt-catalog-build.*'))
            return result, calls, leftovers

    def test_release_versions_match_the_package(self):
        manifest = (ROOT / 'Cargo.toml').read_text()
        version = re.search(r'name = "volt-ui-kit"\nversion = "([^"]+)"', manifest).group(1)
        minor = '.'.join(version.split('.')[:2])
        self.assertIn('--version ' + version, SCRIPT.read_text())
        self.assertIn('name = "volt-ui-kit"\nversion = "' + version + '"',
                      (ROOT / 'Cargo.lock').read_text())
        self.assertIn('## ' + version + ' (', (ROOT / 'CHANGELOG.md').read_text())
        self.assertIn(minor + ' / Pre-1.0', (ROOT / 'scripts/generate_docs.cjs').read_text())
        self.assertIn('brand-version">' + minor + '<', (ROOT / 'docs/index.html').read_text())

    def test_help_and_invalid_options_never_build(self):
        result, calls, _ = self.run_launcher(['--help'])
        self.assertEqual(result.returncode, 0)
        self.assertFalse(calls)
        result, calls, _ = self.run_launcher(['--unknown'])
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(calls)

    def test_full_feature_build_launches_the_exact_cargo_artifact(self):
        result, calls, leftovers = self.run_launcher()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('native-gpu,native-desktop,native-notifications', calls)
        self.assertIn('--locked', calls)
        self.assertIn('executed', calls)
        self.assertIn('macOS-only', result.stdout)
        self.assertFalse(leftovers)

    def test_macos_registers_the_signed_bundle_before_opening(self):
        script = SCRIPT.read_text()
        self.assertLess(script.index('codesign --verify'), script.index('lsregister -f'))
        self.assertLess(script.index('lsregister -f'), script.index('/usr/bin/open -W'))

    def test_check_builds_without_launching(self):
        result, calls, leftovers = self.run_launcher(['--check'])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn('executed', calls)
        self.assertFalse(leftovers)

    def test_failed_build_does_not_launch_a_stale_executable(self):
        result, calls, leftovers = self.run_launcher(fail=True)
        self.assertEqual(result.returncode, 9)
        self.assertNotIn('executed', calls)
        self.assertFalse(leftovers)
