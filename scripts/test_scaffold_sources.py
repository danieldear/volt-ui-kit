"""Starter CI must compile generated Rust, never launch apps or install launchers."""
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import check_scaffold_sources as checker


class ScaffoldCheckTests(unittest.TestCase):
    def test_all_templates_and_desktop_variants_compile_from_an_isolated_locked_fixture(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'Cargo.lock').write_text('version = 4\n', encoding='utf-8')
            generated, phases, fixtures = [], [], []

            def command(args, **kwargs):
                self.assertTrue(kwargs['check'])
                if args[0] != 'cargo':
                    self.assertEqual(args[1], 'new')
                    project = Path(args[2]); generated.append(project)
                    project.mkdir(); (project / 'src').mkdir()
                    app_id = 'dev.volt.' + project.name.replace('-', '_')
                    (project / 'src/main.rs').write_text(f'fn main() {{}} // .app_id("{app_id}")\n', encoding='utf-8')
                    packaging = project / 'packaging/linux'; packaging.mkdir(parents=True)
                    (packaging / f'{app_id}.desktop').write_text(f'Exec={project.name}\nTerminal=false\nStartupWMClass={app_id}\nIcon={app_id}\n', encoding='utf-8')
                    (packaging / f'{app_id}.svg').write_text('<svg/>', encoding='utf-8')
                else:
                    fixture = Path(args[args.index('--manifest-path') + 1]).parent
                    fixtures.append(fixture)
                    self.assertEqual(len(generated), 10)
                    manifest = (fixture / 'Cargo.toml').read_text(encoding='utf-8')
                    self.assertEqual(manifest.count('[[bin]]'), 10)
                    if args[1] == 'update':
                        self.assertEqual(phases, [])
                        self.assertIn('--workspace', args); self.assertIn('--offline', args)
                        self.assertEqual((fixture / 'Cargo.lock').read_text(), 'version = 4\n')
                        (fixture / 'Cargo.lock').write_text('version = 4\n# prepared\n')
                    else:
                        self.assertEqual(phases, ['update'])
                        for flag in ('--locked', '--offline', '--all-targets'): self.assertIn(flag, args)
                        self.assertIn('# prepared', (fixture / 'Cargo.lock').read_text())
                    phases.append(args[1])

            with patch.object(checker, 'ROOT', root), patch.object(checker.subprocess, 'run', side_effect=command), patch('builtins.print'):
                checker.check(root / 'cargo-volt')
            self.assertEqual(phases, ['update', 'check'])
            self.assertEqual((root / 'Cargo.lock').read_text(), 'version = 4\n')
            self.assertTrue(all(not f.exists() for f in fixtures))

    def test_generation_failure_aborts_and_removes_the_fixture(self):
        projects = []
        def fail(command, **kwargs):
            projects.append(Path(command[2]))
            raise subprocess.CalledProcessError(1, command)
        with patch.object(checker.subprocess, 'run', side_effect=fail):
            with self.assertRaises(subprocess.CalledProcessError): checker.check(Path('/not-launched/cargo-volt'))
        self.assertEqual(len(projects), 1)
        self.assertFalse(projects[0].parent.exists())

class LinuxExampleLauncherTests(unittest.TestCase):
    def test_demo_launchers_match_native_app_ids_without_dialog_or_shell_wrappers(self):
        for name in ('catalog', 'northbound'):
            app_id = 'dev.volt.' + name
            source = (checker.ROOT / f'examples/{name}.rs').read_text(encoding='utf-8')
            self.assertIn(f'.app_id("{app_id}")', source)
            launcher = (checker.ROOT / f'examples/linux/{app_id}.desktop').read_text(encoding='utf-8')
            self.assertIn(f'Exec=volt-{name}\n', launcher)
            self.assertIn(f'StartupWMClass={app_id}\n', launcher)
            self.assertIn('Terminal=false\n', launcher)
            self.assertNotIn('sh -c', launcher)
            self.assertIn(f'Icon={app_id}\n', launcher)
            self.assertIn('<svg', (checker.ROOT / f'examples/linux/{app_id}.svg').read_text())
