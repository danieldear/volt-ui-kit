#!/usr/bin/env python3
"""Low-disk Git identity correction. Preview by default; never clones or runs GC.

Only explicitly selected local branches are rewritten. Existing trees/blobs are
reused; original tips are protected by local backup refs. Push and restore are
separate explicit operations. Requires Python 3.10+ and Git 2.29+.
"""
import argparse
from collections import Counter
from contextlib import contextmanager
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import uuid

MIN_FREE = 64 * 1024 * 1024
MAX_COMMIT = 16 * 1024 * 1024
MAX_METADATA = 256 * 1024 * 1024
MAX_COMMITS = 200_000
OID = re.compile(r'(?:[0-9a-f]{40}|[0-9a-f]{64})\Z')
IDENTITY = re.compile(rb'(.+) <([^<>\r\n]*)> (-?\d+ [+-]\d{4})\Z')
COAUTHOR = re.compile(rb'^Co-authored-by:[^\r\n]*(?:\r?\n|$)', re.I | re.M)


class RewriteError(RuntimeError):
    pass


def git(repo, *args, data=None, check=True):
    env = dict(os.environ, GIT_OPTIONAL_LOCKS='0', GIT_NO_REPLACE_OBJECTS='1', GIT_NO_LAZY_FETCH='1')
    result = subprocess.run(['git', '-c', 'gc.auto=0', '-c', 'maintenance.auto=false',
                             '-C', str(repo), *args], input=data, capture_output=True, env=env)
    if check and result.returncode:
        error = result.stderr.decode(errors='replace')[:1200]
        error = re.sub(r'(https?://)[^/@\s]+@', r'\1[redacted]@', error)
        raise RewriteError(f'Git {args[0]} failed: {error.strip()}')
    return result.stdout if check else result


def text(repo, *args):
    return git(repo, *args).decode().strip()


def validate_identity(name, email):
    if not name or not email or len(name.encode()) > 512 or len(email.encode()) > 512:
        raise RewriteError('target identity must be nonempty and at most 512 bytes per field')
    if any(ord(c) < 32 or ord(c) == 127 or c in '<>' for c in name + email) or any(c.isspace() for c in email):
        raise RewriteError('identity contains an invalid header character')


def fields(raw):
    header, separator, message = raw.partition(b'\n\n')
    if not separator:
        raise RewriteError('malformed commit: no header/body separator')
    groups = []
    for line in header.split(b'\n'):
        if line.startswith(b' '):
            if not groups:
                raise RewriteError('malformed multiline commit header')
            groups[-1] += b'\n' + line
        else:
            groups.append(line)
    return groups, message


def transform_commit(raw, parent_map, source_emails, name, email, coauthor_emails, remove_all):
    groups, message = fields(raw)
    updated = []
    info = {'identities_changed': 0, 'coauthors_removed': 0, 'signatures_removed': 0}
    for group in groups:
        key, sep, value = group.partition(b' ')
        if not sep:
            raise RewriteError('malformed commit header')
        if key == b'parent':
            old = value.decode('ascii')
            group = b'parent ' + parent_map.get(old, old).encode('ascii')
        elif key in {b'author', b'committer'}:
            match = IDENTITY.fullmatch(value)
            if not match:
                raise RewriteError('unrecognized author/committer header; refusing to guess')
            if match[2].lower() in source_emails:
                group = key + b' ' + name + b' <' + email + b'> ' + match[3]
                info['identities_changed'] += group != key + b' ' + value
        updated.append(group)

    def remove(match):
        line = match.group()
        identity = re.search(rb'<([^<>\r\n]+)>[ \t]*(?:\r?\n)?\Z', line)
        if remove_all or (identity and identity[1].lower() in coauthor_emails):
            info['coauthors_removed'] += 1
            return b''
        return line

    message = COAUTHOR.sub(remove, message)
    rewritten = b'\n'.join(updated) + b'\n\n' + message
    if rewritten == raw:
        return raw, info
    # Signatures cannot attest a changed commit. No keys are accessed/re-signing attempted.
    clean = []
    for group in updated:
        if group.partition(b' ')[0] in {b'gpgsig', b'gpgsig-sha256', b'mergetag'}:
            info['signatures_removed'] += 1
        else:
            clean.append(group)
    return b'\n'.join(clean) + b'\n\n' + message, info


def branch_ref(repo, branch):
    ref = branch if branch.startswith('refs/heads/') else 'refs/heads/' + branch
    if branch.startswith('refs/') and not branch.startswith('refs/heads/'):
        raise RewriteError('only local branch refs are supported; tags are intentionally untouched')
    git(repo, 'check-ref-format', ref)
    return ref


def checkout(repo):
    root = Path(text(repo, 'rev-parse', '--show-toplevel')).resolve()
    directory = Path(text(root, 'rev-parse', '--absolute-git-dir')).resolve()
    if text(root, 'rev-parse', '--is-shallow-repository') != 'false':
        raise RewriteError('shallow repositories are unsupported; obtain complete history first')
    if len(git(root, 'worktree', 'list', '--porcelain').split(b'worktree ')) != 2:
        raise RewriteError('linked worktrees are unsupported; do not rewrite while other worktrees exist')
    if git(root, 'for-each-ref', '--format=%(refname)', 'refs/replace/').strip():
        raise RewriteError('replace refs exist; review/remove them explicitly before rewriting')
    if (directory / 'info/grafts').exists() or (directory / 'objects/info/alternates').exists():
        raise RewriteError('grafts/alternate object databases are unsupported')
    promisor = git(root, 'config', '--get-regexp', r'^(extensions\.partialclone|remote\..*\.promisor)$', check=False)
    if promisor.stdout.strip():
        raise RewriteError('partial/promisor clones are unsupported; no lazy downloads are performed')
    for marker in ['MERGE_HEAD', 'CHERRY_PICK_HEAD', 'REVERT_HEAD', 'rebase-merge', 'rebase-apply', 'BISECT_LOG']:
        if (directory / marker).exists():
            raise RewriteError(f'finish the in-progress Git operation first: {marker}')
    return root, directory


def require_clean(repo):
    if git(repo, 'status', '--porcelain=v1', '--untracked-files=normal').strip():
        raise RewriteError('checkout has pending changes; commit or separately preserve them first; no automatic stash/reset')


def commits(repo, refs):
    ids = text(repo, 'rev-list', '--reverse', '--topo-order', f'--max-count={MAX_COMMITS + 1}', *refs).splitlines()
    if len(ids) > MAX_COMMITS:
        raise RewriteError(f'more than {MAX_COMMITS} commits; use a separately reviewed large-repo migration')
    # Read metadata only. No file blobs, checkout copies, export files or new packfiles.
    result = []
    total = 0
    env = dict(os.environ, GIT_NO_REPLACE_OBJECTS='1', GIT_NO_LAZY_FETCH='1')
    with subprocess.Popen(['git', '-C', str(repo), 'cat-file', '--batch'],
                          stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, env=env) as reader:
        try:
            for oid in ids:
                reader.stdin.write(oid.encode() + b'\n')
                reader.stdin.flush()
                header = reader.stdout.readline().split()
                if len(header) != 3 or header[1] != b'commit':
                    raise RewriteError('missing or invalid commit object')
                size = int(header[2])
                total += size
                if size > MAX_COMMIT or total > MAX_METADATA:
                    raise RewriteError('commit metadata exceeds safety budget; no rewrite performed')
                raw = reader.stdout.read(size)
                if len(raw) != size or reader.stdout.read(1) != b'\n':
                    raise RewriteError('truncated commit object')
                result.append((oid, raw))
        finally:
            reader.stdin.close()
            reader.terminate()
    return result


def remote_identity(repo, remote):
    if remote.startswith('-') or remote not in text(repo, 'remote').splitlines():
        raise RewriteError('choose an existing named remote')
    urls = git(repo, 'remote', 'get-url', '--push', '--all', remote).splitlines()
    if len(urls) != 1:
        raise RewriteError('multiple push URLs are unsupported')
    fetch_urls = git(repo, 'remote', 'get-url', '--all', remote).splitlines()
    if fetch_urls != urls:
        raise RewriteError('different fetch/push URLs are unsupported; use a remote with one identical endpoint')
    # Never persist an embedded URL password/token in a receipt.
    return hashlib.sha256(urls[0]).hexdigest()


def remote_tips(repo, remote, refs):
    output = text(repo, 'ls-remote', '--refs', remote, *refs)
    tips = {ref: None for ref in refs}
    for line in output.splitlines():
        oid, ref = line.split('\t')
        if ref in tips:
            tips[ref] = oid
    return tips


def atomic_json(path, data):
    temp = path.with_suffix('.tmp')
    fd = os.open(temp, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    try:
        with os.fdopen(fd, 'w') as stream:
            json.dump(data, stream, indent=2)
            stream.write('\n')
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temp, path)
    finally:
        temp.unlink(missing_ok=True)


@contextmanager
def lock(directory):
    path = directory / 'identity-rewrite.lock'
    try:
        fd = os.open(path, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    except FileExistsError as error:
        raise RewriteError(f'rewrite lock exists: {path}; verify no script is running before removing a stale lock') from error
    try:
        os.write(fd, str(os.getpid()).encode())
        os.close(fd)
        yield
    finally:
        path.unlink(missing_ok=True)


def update_refs(repo, updates, backups=()):
    lines = ['start']
    for ref, old in backups:
        lines.append(f'create {ref} {old}')
    for ref, new, old in updates:
        lines.append(f'update {ref} {new} {old}')
    lines += ['prepare', 'commit', '']
    git(repo, 'update-ref', '-m', 'identity correction with shared-object backup refs', '--stdin', data='\n'.join(lines).encode())


def rewrite(args):
    repo, directory = checkout(args.repo)
    validate_identity(args.to_name, args.to_email)
    sources = {value.lower().encode() for value in args.from_email}
    coauthors = {value.lower().encode() for value in args.remove_coauthor_email}
    for value in args.from_email + args.remove_coauthor_email:
        validate_identity('source', value)
    refs = sorted({branch_ref(repo, branch) for branch in args.branch})
    old_refs = {ref: text(repo, 'rev-parse', '--verify', ref) for ref in refs}
    original = commits(repo, refs)
    # Predict changes without writing objects. A child changes when its parent changes.
    planned = {}
    totals = Counter(commits_to_change=0, identities_changed=0, coauthors_removed=0, signatures_removed=0)
    authors = Counter()
    for oid, raw in original:
        changed, info = transform_commit(raw, planned, sources, args.to_name.encode(), args.to_email.encode(), coauthors, args.remove_all_coauthors)
        for group in fields(changed)[0]:
            if group.startswith(b'author '):
                match = IDENTITY.fullmatch(group[7:])
                authors[match[2].decode(errors='replace')] += 1
        if changed != raw:
            planned[oid] = '0' * len(oid)  # valid placeholder, not a written object
            totals['commits_to_change'] += 1
        totals.update(info)
    blockers = []
    try:
        require_clean(repo)
    except RewriteError as error:
        blockers.append(str(error))
    estimated = MIN_FREE + sum(max(8192, len(raw) * 3) for _, raw in original)
    free = shutil.disk_usage(directory).free
    if free < estimated:
        blockers.append(f'insufficient free space: {free} available; need at least {estimated} bytes of estimated headroom')
    if args.expect_author_email and set(authors) != {args.expect_author_email}:
        blockers.append('planned authors do not match --expect-author-email; inspect all source identities')
    branches = text(repo, 'for-each-ref', '--format=%(refname)', 'refs/heads/').splitlines()
    tags = text(repo, 'for-each-ref', '--format=%(refname)', 'refs/tags/').splitlines()
    result = dict(totals, applied=False, selected_branches=refs, commits_inspected=len(original),
                  resulting_author_emails=dict(authors), estimated_new_metadata_bytes=estimated - MIN_FREE,
                  estimated_headroom_bytes=estimated,
                  free_bytes=free, unselected_branches=sorted(set(branches) - set(refs)),
                  untouched_tags=tags, blockers=blockers)
    print(json.dumps(result, indent=2))
    if not args.apply:
        return result
    if blockers:
        raise RewriteError('rewrite refused: ' + '; '.join(blockers))
    if not totals['commits_to_change']:
        return result
    # Capture exact remote tips before changing local refs. No network access in preview.
    identity = remote_identity(repo, args.remote)
    remote = remote_tips(repo, args.remote, refs)
    for ref, remote_oid in remote.items():
        if remote_oid:
            if git(repo, 'merge-base', '--is-ancestor', remote_oid, old_refs[ref], check=False).returncode:
                raise RewriteError(f'{ref}: remote contains history not present locally; fetch and reconcile first')
    with lock(directory):
        require_clean(repo)
        if any(text(repo, 'rev-parse', ref) != old for ref, old in old_refs.items()):
            raise RewriteError('branch changed while preparing; no rewrite performed')
        storage = directory / 'identity-rewrite'
        if storage.is_symlink():
            raise RewriteError('receipt directory must not be a symlink')
        storage.mkdir(mode=0o700, exist_ok=True)
        run_id = datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ') + '-' + uuid.uuid4().hex[:8]
        run = storage / run_id
        run.mkdir(mode=0o700)
        receipt_path = run / 'receipt.json'
        receipt = {'version': 1, 'git_dir': str(directory), 'state': 'preparing',
                   'remote': args.remote, 'remote_identity': identity, 'run_id': run_id,
                   'refs': {ref: {'old': old, 'remote_old': remote[ref], 'backup_ref':
                                  f'refs/identity-backups/{run_id}/{ref.removeprefix("refs/")}' }
                            for ref, old in old_refs.items()}}
        atomic_json(receipt_path, receipt)
        mapping = {}
        try:
            for old, raw in original:
                changed, _ = transform_commit(raw, mapping, sources, args.to_name.encode(), args.to_email.encode(), coauthors, args.remove_all_coauthors)
                mapping[old] = old if changed == raw else git(repo, 'hash-object', '-t', 'commit', '-w', '--stdin', data=changed).decode().strip()
            for ref, record in receipt['refs'].items():
                record['new'] = mapping[record['old']]
                record['tree'] = text(repo, 'rev-parse', record['old'] + '^{tree}')
                if text(repo, 'rev-parse', record['new'] + '^{tree}') != record['tree']:
                    raise RewriteError('tree mismatch; refusing branch update')
            receipt['commit_map'] = mapping
            receipt['state'] = 'prepared'
            atomic_json(receipt_path, receipt)
            require_clean(repo)
            update_refs(repo, [(ref, r['new'], r['old']) for ref, r in receipt['refs'].items()],
                        [(r['backup_ref'], r['old']) for r in receipt['refs'].values()])
            receipt['state'] = 'rewritten'
            atomic_json(receipt_path, receipt)
            if args.set_local_identity:
                git(repo, 'config', '--local', '--', 'user.name', args.to_name)
                git(repo, 'config', '--local', '--', 'user.email', args.to_email)
        except Exception:
            print(f'Operation interrupted. Inspect receipt and backup refs before retrying: {receipt_path}')
            raise
    result.update(applied=True, receipt=str(receipt_path))
    print(f'Local rewrite complete. No push or GC performed. Receipt: {receipt_path}')
    return result


def read_receipt(repo, directory, path):
    if path.stat().st_size > MAX_METADATA:
        raise RewriteError('receipt exceeds safety budget')
    receipt = json.loads(path.read_text())
    if not isinstance(receipt, dict) or receipt.get('version') != 1 or receipt.get('git_dir') != str(directory):
        raise RewriteError('receipt belongs to a different repository or unsupported version')
    if receipt.get('state') not in {'prepared', 'rewritten', 'pushed', 'restored'}:
        raise RewriteError('rewrite was not prepared; inspect the receipt manually')
    if not isinstance(receipt.get('refs'), dict) or not receipt['refs']:
        raise RewriteError('receipt has no branches')
    if not re.fullmatch(r'[A-Za-z0-9-]+', receipt.get('run_id', '')):
        raise RewriteError('invalid run identifier')
    if not isinstance(receipt.get('remote_identity'), str) or not re.fullmatch(r'[0-9a-f]{64}', receipt['remote_identity']):
        raise RewriteError('invalid remote identity digest')
    for ref, record in receipt['refs'].items():
        if not isinstance(record, dict):
            raise RewriteError('invalid branch record')
        if record.get('remote_old') is not None and not OID.fullmatch(str(record['remote_old'])):
            raise RewriteError('invalid remote lease')
        if not ref.startswith('refs/heads/') or branch_ref(repo, ref) != ref:
            raise RewriteError('invalid branch in receipt')
        for key in ['old', 'new', 'tree']:
            if not isinstance(record.get(key), str) or not OID.fullmatch(record[key]):
                raise RewriteError('invalid object ID in receipt')
        expected = f'refs/identity-backups/{receipt["run_id"]}/{ref.removeprefix("refs/")}'
        if record.get('backup_ref') != expected:
            raise RewriteError('unexpected backup ref in receipt')
        git(repo, 'check-ref-format', expected)
        if text(repo, 'rev-parse', expected) != record['old']:
            raise RewriteError('original backup ref is missing or changed')
    return receipt


def followup(args):
    repo, directory = checkout(args.repo)
    receipt = read_receipt(repo, directory, args.receipt)
    for ref, record in receipt['refs'].items():
        if text(repo, 'rev-parse', ref) != record['new']:
            raise RewriteError(f'{ref} changed after the rewrite; refusing to discard later work')
        if text(repo, 'rev-parse', record['new'] + '^{tree}') != record['tree']:
            raise RewriteError('tree readback mismatch')
    print(f'{args.command}: {len(receipt["refs"])} explicitly recorded branch(es); apply={args.apply}')
    if not args.apply:
        print('Preview only. No local refs or remote branches changed.')
        return receipt
    with lock(directory):
        require_clean(repo)
        if args.command == 'restore':
            update_refs(repo, [(ref, r['old'], r['new']) for ref, r in receipt['refs'].items()])
            receipt['state'] = 'restored'
            print('Local branches restored. Remote and backup refs untouched; corrected local identity kept.')
        else:
            remote = receipt['remote']
            if remote_identity(repo, remote) != receipt['remote_identity']:
                raise RewriteError('push remote changed; refusing to use a different endpoint')
            actual = remote_tips(repo, remote, list(receipt['refs']))
            for ref, record in receipt['refs'].items():
                if actual[ref] != record['remote_old']:
                    raise RewriteError(f'{ref}: remote changed since rewrite; lease refused')
            leases = [f'--force-with-lease={ref}:{r["remote_old"] or ""}' for ref, r in receipt['refs'].items()]
            specs = [f'{r["new"]}:{ref}' for ref, r in receipt['refs'].items()]
            # Never --mirror/--all/--tags. Literal OIDs prevent a concurrent local
            # commit from silently being included. Atomic push is mandatory.
            git(repo, 'push', '--atomic', '--no-follow-tags', *leases, remote, *specs)
            receipt['state'] = 'pushed'
            print('Selected branches pushed with exact leases. No tags or backup refs pushed.')
        atomic_json(args.receipt, receipt)
    return receipt


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest='command', required=True)
    rewrite_parser = sub.add_parser('rewrite', help='preview, or explicitly apply, a local metadata rewrite')
    rewrite_parser.add_argument('--repo', type=Path, default=Path('.'))
    rewrite_parser.add_argument('--branch', action='append', required=True, help='local branch; repeat for every retained branch')
    rewrite_parser.add_argument('--from-email', action='append', required=True, help='only these identities are corrected (case-insensitive)')
    rewrite_parser.add_argument('--to-name', required=True)
    rewrite_parser.add_argument('--to-email', required=True)
    rewrite_parser.add_argument('--remove-coauthor-email', action='append', default=[])
    rewrite_parser.add_argument('--remove-all-coauthors', action='store_true', help='explicitly remove all Co-authored-by lines; do not erase legitimate human credit')
    rewrite_parser.add_argument('--expect-author-email', help='refuse if any selected commit would keep another author email')
    rewrite_parser.add_argument('--remote', default='origin')
    rewrite_parser.add_argument('--set-local-identity', action='store_true', help='also set this repo local user.name/email; never global config')
    rewrite_parser.add_argument('--apply', action='store_true', help='actually rewrite local refs; never pushes')
    for command in ['push', 'restore']:
        operation = sub.add_parser(command, help=f'preview or explicitly {command} using a saved receipt')
        operation.add_argument('--repo', type=Path, default=Path('.'))
        operation.add_argument('--receipt', type=Path, required=True)
        operation.add_argument('--apply', action='store_true')
    args = parser.parse_args(argv)
    return rewrite(args) if args.command == 'rewrite' else followup(args)


if __name__ == '__main__':
    try:
        main()
    except (RewriteError, OSError, ValueError) as error:
        raise SystemExit(f'ERROR: {error}')
