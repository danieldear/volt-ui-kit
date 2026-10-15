"""Tiny disposable repositories only: never rewrite this checkout in tests."""
import contextlib
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import rewrite_git_identity as tool


class IdentityTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name) / 'repo'
        self.root.mkdir()
        self.git('init', '-b', 'main')
        self.git('config', 'user.name', 'Old Identity')
        self.git('config', 'user.email', 'old@example.test')
        self.git('config', 'commit.gpgsign', 'false')
        (self.root / 'LICENSE').write_text('Preserve original license notices.\n')
        self.git('add', 'LICENSE')
        self.git('commit', '-m', 'Initial\n\nCo-Authored-By: Assistant <bot@example.test>')
        self.git('checkout', '-b', 'topic')
        (self.root / 'app.txt').write_text('same source, different attribution\n')
        self.git('add', '.')
        self.git('commit', '-m', 'Feature\n\nCo-authored-by: Human <human@example.test>')
        self.git('checkout', 'main')
        self.git('merge', '--no-ff', 'topic', '-m', 'Merge topic')
        self.git('tag', '-a', 'v-test', '-m', 'Original tag metadata')
        self.remote = Path(self.tmp.name) / 'remote.git'
        subprocess.run(['git', 'init', '--bare', str(self.remote)], check=True, capture_output=True)
        self.git('remote', 'add', 'origin', str(self.remote))
        self.git('push', 'origin', 'main', 'topic')
        self.head = self.git('rev-parse', 'main').strip()
        self.tree = self.git('rev-parse', 'main^{tree}').strip()
        self.tag = self.git('rev-parse', 'refs/tags/v-test').strip()

    def git(self, *args):
        env = dict(os.environ, GIT_CONFIG_NOSYSTEM='1', GIT_CONFIG_GLOBAL=os.devnull)
        return subprocess.check_output(['git', '-C', str(self.root), *args], stderr=subprocess.PIPE, env=env).decode()

    def run_tool(self, *extra, apply=False):
        args = ['rewrite', '--repo', str(self.root), '--branch', 'main', '--branch', 'topic',
                '--from-email', 'old@example.test', '--to-name', 'New Identity',
                '--to-email', 'new@example.test', '--remove-coauthor-email', 'bot@example.test']
        if apply:
            args.append('--apply')
        with contextlib.redirect_stdout(io.StringIO()):
            return tool.main(args + list(extra))

    def receipt(self):
        return next((self.root / '.git/identity-rewrite').glob('*/receipt.json'))

    def test_preview_is_read_only_even_when_dirty(self):
        (self.root / 'untracked.txt').write_text('not committed')
        result = self.run_tool()
        self.assertFalse(result['applied'])
        self.assertEqual(result['commits_to_change'], 3)
        self.assertTrue(result['blockers'])
        self.assertEqual(self.git('rev-parse', 'main').strip(), self.head)
        self.assertFalse((self.root / '.git/identity-rewrite').exists())

    def test_apply_refuses_dirty_checkout_without_writing_receipt(self):
        (self.root / 'LICENSE').write_text('pending user edits')
        with self.assertRaises(tool.RewriteError):
            self.run_tool(apply=True)
        self.assertEqual(self.git('rev-parse', 'main').strip(), self.head)
        self.assertFalse((self.root / '.git/identity-rewrite').exists())

    def test_apply_preserves_trees_merges_dates_license_and_human_credit(self):
        dates = self.git('log', '--format=%at:%ct', 'main')
        count = self.git('rev-list', '--count', 'main')
        result = self.run_tool(apply=True)
        self.assertTrue(result['applied'])
        self.assertNotEqual(self.git('rev-parse', 'main').strip(), self.head)
        self.assertEqual(self.git('rev-parse', 'main^{tree}').strip(), self.tree)
        self.assertEqual(self.git('log', '--format=%at:%ct', 'main'), dates)
        self.assertEqual(self.git('rev-list', '--count', 'main'), count)
        self.assertEqual(len(self.git('show', '-s', '--format=%P', 'main').split()), 2)
        self.assertEqual(self.git('status', '--porcelain'), '')
        messages = self.git('log', '--format=%B', 'main')
        self.assertNotIn('bot@example.test', messages)
        self.assertIn('human@example.test', messages)
        self.assertEqual(set(self.git('log', '--format=%ae%n%ce', 'main').splitlines()), {'new@example.test'})
        self.assertEqual(self.git('rev-parse', 'refs/tags/v-test').strip(), self.tag)
        self.assertEqual((self.root / 'LICENSE').read_text(), 'Preserve original license notices.\n')
        receipt = json.loads(self.receipt().read_text())
        self.assertEqual(self.git('rev-parse', receipt['refs']['refs/heads/main']['backup_ref']).strip(), self.head)
        self.assertEqual(subprocess.check_output(['git', '--git-dir', str(self.remote), 'rev-parse', 'main']).decode().strip(), self.head)

    def test_no_new_blobs_trees_packs_or_bundles_are_created(self):
        def objects():
            return {line.split()[0]: line.split()[1] for line in
                    self.git('cat-file', '--batch-all-objects', '--batch-check=%(objectname) %(objecttype)').splitlines()}
        before = objects()
        packs = set((self.root / '.git/objects/pack').glob('*'))
        self.run_tool(apply=True)
        after = objects()
        self.assertTrue(set(before).issubset(after))
        self.assertTrue(all(kind == 'commit' for oid, kind in after.items() if oid not in before))
        self.assertEqual(set((self.root / '.git/objects/pack').glob('*')), packs)
        self.assertFalse(list((self.root / '.git').rglob('*.bundle')))
        receipt = json.loads(self.receipt().read_text())
        mapping = receipt['commit_map']
        for old, new in mapping.items():
            self.assertEqual(self.git('rev-parse', old + '^{tree}'), self.git('rev-parse', new + '^{tree}'))
            old_parents = self.git('show', '-s', '--format=%P', old).split()
            new_parents = self.git('show', '-s', '--format=%P', new).split()
            self.assertEqual(new_parents, [mapping.get(parent, parent) for parent in old_parents])

    def test_noop_rewrite_does_not_create_backups(self):
        with contextlib.redirect_stdout(io.StringIO()):
            result = tool.main(['rewrite', '--repo', str(self.root), '--branch', 'main', '--from-email', 'missing@example.test', '--to-name', 'New', '--to-email', 'new@example.test', '--apply'])
        self.assertFalse(result['applied'])
        self.assertEqual(self.git('rev-parse', 'main').strip(), self.head)
        self.assertFalse((self.root / '.git/identity-rewrite').exists())

    def test_split_fetch_and_push_endpoint_is_refused(self):
        self.git('remote', 'set-url', '--push', 'origin', str(Path(self.tmp.name) / 'different.git'))
        with self.assertRaises(tool.RewriteError):
            self.run_tool(apply=True)
        self.assertEqual(self.git('rev-parse', 'main').strip(), self.head)
        self.assertFalse((self.root / '.git/identity-rewrite').exists())

    def test_unexpected_author_guard_refuses_before_writes(self):
        with self.assertRaises(tool.RewriteError):
            self.run_tool('--expect-author-email', 'someone-else@example.test', apply=True)
        self.assertFalse((self.root / '.git/identity-rewrite').exists())

    def test_explicit_future_identity_configuration_is_local(self):
        self.run_tool('--set-local-identity', apply=True)
        self.assertEqual(self.git('config', '--local', '--get', 'user.name').strip(), 'New Identity')
        self.assertEqual(self.git('config', '--local', '--get', 'user.email').strip(), 'new@example.test')

    def test_low_disk_blocks_before_object_or_receipt_writes(self):
        with patch.object(tool.shutil, 'disk_usage', return_value=type('Usage', (), {'free': 1})()):
            with self.assertRaises(tool.RewriteError):
                self.run_tool(apply=True)
        self.assertFalse((self.root / '.git/identity-rewrite').exists())

    def test_push_is_read_only_until_explicit_apply_and_never_pushes_backups(self):
        self.run_tool(apply=True)
        with contextlib.redirect_stdout(io.StringIO()):
            tool.main(['push', '--repo', str(self.root), '--receipt', str(self.receipt())])
        self.assertEqual(subprocess.check_output(['git', '--git-dir', str(self.remote), 'rev-parse', 'main']).decode().strip(), self.head)
        with contextlib.redirect_stdout(io.StringIO()):
            tool.main(['push', '--repo', str(self.root), '--receipt', str(self.receipt()), '--apply'])
        refs = subprocess.check_output(['git', '--git-dir', str(self.remote), 'for-each-ref', '--format=%(refname)']).decode()
        self.assertNotIn('identity-backups', refs)
        self.assertNotIn('refs/tags/', refs)
        self.assertEqual(subprocess.check_output(['git', '--git-dir', str(self.remote), 'rev-parse', 'main']).decode().strip(), self.git('rev-parse', 'main').strip())

    def test_push_lease_refuses_remote_change_after_rewrite(self):
        self.run_tool(apply=True)
        old_parent = self.git('rev-parse', self.head + '^1').strip()
        subprocess.run(['git', '--git-dir', str(self.remote), 'update-ref', 'refs/heads/main', old_parent], check=True)
        with contextlib.redirect_stdout(io.StringIO()), self.assertRaises(tool.RewriteError):
            tool.main(['push', '--repo', str(self.root), '--receipt', str(self.receipt()), '--apply'])
        self.assertEqual(subprocess.check_output(['git', '--git-dir', str(self.remote), 'rev-parse', 'main']).decode().strip(), old_parent)

    def test_atomic_push_lease_protects_against_a_race_after_precheck(self):
        self.run_tool(apply=True)
        original_git = tool.git
        old_topic = subprocess.check_output(['git', '--git-dir', str(self.remote), 'rev-parse', 'topic']).decode().strip()
        moved = self.git('rev-parse', self.head + '^1').strip()
        def racing_git(repo, *args, **kwargs):
            if args[0] == 'push':
                subprocess.run(['git', '--git-dir', str(self.remote), 'update-ref', 'refs/heads/main', moved], check=True)
            return original_git(repo, *args, **kwargs)
        with patch.object(tool, 'git', side_effect=racing_git), contextlib.redirect_stdout(io.StringIO()), self.assertRaises(tool.RewriteError):
            tool.main(['push', '--repo', str(self.root), '--receipt', str(self.receipt()), '--apply'])
        self.assertEqual(subprocess.check_output(['git', '--git-dir', str(self.remote), 'rev-parse', 'topic']).decode().strip(), old_topic)

    def test_explicit_new_branch_uses_an_absent_remote_lease(self):
        self.git('branch', 'local-only')
        self.run_tool('--branch', 'local-only', apply=True)
        data = json.loads(self.receipt().read_text())
        self.assertIsNone(data['refs']['refs/heads/local-only']['remote_old'])
        with contextlib.redirect_stdout(io.StringIO()):
            tool.main(['push', '--repo', str(self.root), '--receipt', str(self.receipt()), '--apply'])
        self.assertEqual(subprocess.check_output(['git', '--git-dir', str(self.remote), 'rev-parse', 'local-only']).decode().strip(), self.git('rev-parse', 'local-only').strip())

    def test_ref_transaction_is_all_or_nothing_with_backup_creation(self):
        old_topic = self.git('rev-parse', 'topic').strip()
        wrong_old = self.git('rev-parse', self.head + '^1').strip()
        with self.assertRaises(tool.RewriteError):
            tool.update_refs(self.root,
                [('refs/heads/main', old_topic, wrong_old), ('refs/heads/topic', self.head, old_topic)],
                [('refs/identity-backups/test/main', self.head)])
        self.assertEqual(self.git('rev-parse', 'main').strip(), self.head)
        self.assertEqual(self.git('rev-parse', 'topic').strip(), old_topic)
        self.assertEqual(self.git('for-each-ref', '--format=%(refname)', 'refs/identity-backups/'), '')

    def test_restore_uses_compare_and_swap_and_preserves_source(self):
        self.run_tool(apply=True)
        with contextlib.redirect_stdout(io.StringIO()):
            tool.main(['restore', '--repo', str(self.root), '--receipt', str(self.receipt()), '--apply'])
        self.assertEqual(self.git('rev-parse', 'main').strip(), self.head)
        self.assertEqual(self.git('rev-parse', 'main^{tree}').strip(), self.tree)
        self.assertEqual(self.git('status', '--porcelain'), '')

    def test_unrelated_new_commits_block_restore(self):
        self.run_tool(apply=True)
        self.git('commit', '--allow-empty', '-m', 'Work after rewrite')
        after = self.git('rev-parse', 'main').strip()
        with contextlib.redirect_stdout(io.StringIO()), self.assertRaises(tool.RewriteError):
            tool.main(['restore', '--repo', str(self.root), '--receipt', str(self.receipt()), '--apply'])
        self.assertEqual(self.git('rev-parse', 'main').strip(), after)

    def test_partial_selection_leaves_unselected_branch_unchanged(self):
        original_topic = self.git('rev-parse', 'topic').strip()
        with contextlib.redirect_stdout(io.StringIO()):
            tool.main(['rewrite', '--repo', str(self.root), '--branch', 'main', '--from-email', 'old@example.test', '--to-name', 'New', '--to-email', 'new@example.test', '--apply'])
        self.assertEqual(self.git('rev-parse', 'topic').strip(), original_topic)


class CommitTests(unittest.TestCase):
    RAW = b'tree ' + b'a'*40 + b'\nauthor Old <old@example.test> 1 +0000\ncommitter Old <old@example.test> 2 -0500\ngpgsig fake\n continuation\n\nBody\n\nCo-Authored-By: Bot <bot@example.test>\n'

    def test_modified_commit_loses_invalid_signature_and_preserves_dates(self):
        new, info = tool.transform_commit(self.RAW, {}, {b'old@example.test'}, b'New', b'new@example.test', {b'bot@example.test'}, False)
        self.assertNotIn(b'gpgsig', new)
        self.assertIn(b'1 +0000', new)
        self.assertIn(b'2 -0500', new)
        self.assertEqual(info['coauthors_removed'], 1)
        self.assertTrue(new.endswith(b'Body\n\n'))

    def test_unchanged_signature_is_not_stripped(self):
        new, _ = tool.transform_commit(self.RAW, {}, set(), b'New', b'new@example.test', set(), False)
        self.assertEqual(new, self.RAW)

    def test_header_injection_is_rejected(self):
        with self.assertRaises(tool.RewriteError):
            tool.validate_identity('Name\ncommitter Evil', 'x@example.test')


if __name__ == '__main__':
    unittest.main()
