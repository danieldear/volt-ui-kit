# Correcting Git identity without cloning history

`scripts/rewrite_git_identity.py` is a reusable, Python-standard-library tool
for correcting **your own accidental identity aliases** and removing specified
co-author lines. Do not use it to erase legitimate third-party authorship.
License/copyright notices and source files are never edited.

It requires Python 3.10+ and Git 2.29+. No pip/Homebrew dependency is needed.
Every command is a read-only preview unless you add `--apply`. The script may
be run by absolute path against another project with `--repo /path/to/project`.
It never changes global Git configuration or operates on another repository
implicitly.

## Low-disk design

```text
Before: selected branch ---> original commits ---> trees / file blobs

After:  selected branch ---> corrected commits ---+
                                                +--> SAME trees / file blobs
        local backup ref --> original commits ---+
```

Only changed commit objects and a small JSON receipt are written. There is no
clone, worktree copy, bundle, blob export, new packfile or automatic garbage
collection. Backup refs share the existing object database: they are pointers,
not another copy of the files/history. They protect the old commits from normal
GC but are **not** an independent backup against disk failure.

The preview shows a conservative metadata estimate plus a 64 MiB free-space
margin. It refuses insufficient space. This is an estimate, not a disk-space
reservation: stop concurrent large writes and Git jobs before applying.
For unusually large histories, the tool refuses more than 200,000 commits,
16 MiB per commit or 256 MiB of total commit metadata rather than consuming
unbounded memory/disk. No existing recovery data is pruned automatically.

## Prepare safely

1. Save and commit pending work before applying, including this script if it
   is inside the repository. Review what you commit: don't add secrets or test
   artifacts. The tool refuses tracked changes and untracked non-ignored files;
   it never stashes, resets or deletes your work. Ignored local files are untouched.
2. Coordinate a history-rewrite maintenance window with other contributors.
   Inspect `git branch -a` and `git tag`; choose all retained branches that must
   share the corrected ancestry. Don't later merge an old, unrewritten branch
   into the corrected history.
3. Fetch the remote normally and reconcile any remote-only commits first.
   Applying records the exact remote tips and refuses divergent/ahead remote
   history. Preview is local-only and does not fetch or access the network.
4. Choose exact old email addresses to map, the target name/email, and which
   co-author lines to remove. GitHub associates commits with email addresses;
   use the no-reply address shown in your GitHub email settings if desired.

## Preview the change

Run from the target repository, or supply `--repo` explicitly:

```sh
python3 /path/to/volt-ui-kit/scripts/rewrite_git_identity.py rewrite \
  --repo /path/to/project \
  --branch main \
  --branch my-feature \
  --from-email old@example.test \
  --to-name my-github-login \
  --to-email 12345+my-github-login@users.noreply.github.com \
  --remove-coauthor-email bot@example.test \
  --expect-author-email 12345+my-github-login@users.noreply.github.com \
  --set-local-identity
```

Replace the example identities and branch names; do not paste them unchanged.
Repeat `--from-email`, `--remove-coauthor-email` or `--branch` as needed.
`--expect-author-email` is an optional guard for a genuinely single-author
project. Omit it when legitimate other authors must remain.

The preview reports affected commits, identity fields, removed co-author lines,
invalidated signature headers, planned authors, unselected branches/tags and
blockers. It creates no backup refs or receipt and writes no commit objects.

`--remove-coauthor-email` preserves every other co-author. To deliberately
remove **all** `Co-authored-by:` lines instead, use `--remove-all-coauthors`.
Email matching is ASCII case-insensitive. This edits matching lines in commit messages,
not source files, licenses, PR bodies or issue conversations.

## Apply locally, then inspect

Repeat the reviewed command with **`--apply` appended**. This is not a push.

- Exact-email matches are corrected in author and committer fields. Unrelated
  authors and GitHub's service committer identity are not silently reassigned.
- Author/committer dates, every commit's tree, merge parent order and commit
  count are preserved. Empty commits are not pruned or squashed.
- Changed commit IDs invalidate their original cryptographic signatures.
  `gpgsig`, `gpgsig-sha256` and embedded `mergetag` headers are removed from
  changed commits; no keys are accessed and commits are not re-signed.
- Selected branch updates and local backup refs are committed as one atomic,
  compare-and-swap ref transaction. A concurrent branch update causes failure.
- `--set-local-identity` sets `user.name` and `user.email` **only in this repo**
  after a successful rewrite. This prevents the accidental alias recurring.
- A receipt is saved under `.git/identity-rewrite/<run-id>/receipt.json` with
  original/new IDs, the commit mapping, backup refs and original remote leases.
  It is local Git metadata, not a tracked/public project file. The remote URL
  is represented by a digest so embedded credentials aren't copied into receipts.

Inspect selected branches (not `--all`, which intentionally includes backups):

```sh
git log main --format='%h %an <%ae>'
git log main --format='%B' --grep='Co-authored-by:' -i
git status --short
```

The script refuses shallow/partial/alternate-object clones, replacement refs,
grafts, linked worktrees and unfinished merge/rebase/cherry-pick operations.
Use a separately reviewed migration for those cases. A named remote with one
identical fetch/push URL is required; split or multiple push endpoints are
refused. Remote configuration is trusted local Git configuration, not a sandbox.

## Push separately, with exact leases

Use the actual receipt path printed by the rewrite:

```sh
python3 /path/to/volt-ui-kit/scripts/rewrite_git_identity.py push \
  --repo /path/to/project \
  --receipt /path/to/project/.git/identity-rewrite/RUN-ID/receipt.json
# Inspect the preview, then repeat with --apply appended.
```

The explicit push uses `--atomic`, exact `--force-with-lease=REF:OLD-OID`
expectations and literal new commit IDs. It refuses a changed remote endpoint,
changed local branch, dirty checkout or changed remote branch. It never uses
`--mirror`, `--all`, `--tags`, unconditional `--force` or pushes backup refs.
An unsupported atomic push or protected branch fails rather than weakening
server protection. A newly created local branch can be published only when
its recorded remote lease expected that branch not to exist.

Stop other writers during this maintenance window. If the remote advances,
**do not bypass the lease**. Reconcile the new work and prepare a new reviewed
rewrite. Don't recreate the receipt by guessing remote hashes.

## Restore local branches

Before making further commits:

```sh
python3 /path/to/volt-ui-kit/scripts/rewrite_git_identity.py restore \
  --repo /path/to/project \
  --receipt /path/to/project/.git/identity-rewrite/RUN-ID/receipt.json
# Inspect the preview, then repeat with --apply appended.
```

Restore checks that local branches still point at the rewritten tips and
atomically restores their original IDs. It refuses to discard later commits.
It does **not** restore/force-push a remote and keeps the corrected local Git
identity. If you already pushed, remote rollback is a separate coordinated
maintenance operation with new leases, not an automatic action.

On interruption, inspect the receipt and selected refs before retrying. If
refs changed, the backup refs were created in the same transaction and can
recover the originals. If refs never changed, original history remains in
place; newly written commit objects may simply be unreachable. A stale
`.git/identity-rewrite.lock` must only be removed after confirming no migration
process is running. Receipts use file fsync; no power-loss/drive-failure
recovery guarantee is implied.

## Important limits

- This is **identity correction**, not secret erasure. Local backup refs,
  remote-tracking refs, reflogs and unselected branches retain old history.
  Don't push them with a later mirror operation.
- Tags (including annotated tagger identities/signatures), notes, hidden
  GitHub PR refs, PR/issue authors and GitHub activity are not rewritten. If
  tags must move, recreate/re-sign them in a separately reviewed operation.
- Old commit links and signed-commit verification change. Other clones must
  resynchronize or reclone; merging stale ancestry reintroduces old metadata.
- GitHub contributor statistics are cached and can take about 24 hours to
  refresh. Correcting commits does not delete an account's historic PR/issue
  activity. See [GitHub's contributor documentation](https://docs.github.com/en/repositories/viewing-activity-and-data-for-your-repository/viewing-a-projects-contributors).
- Backup refs keep recovery possible without copying original blobs. Keep
  them until you have verified the rewrite. Removing them/reflogs or pruning
  objects later is an explicit owner operation, never automatic here.

## Regression tests

```sh
python3 -m unittest discover -s /path/to/volt-ui-kit/scripts -p 'test_git_identity.py'
```

Tests create only tiny temporary repositories and a local disposable bare
remote. They cover metadata-only writes, preserved trees/merge topology/dates,
selective human credit preservation, dirty/low-disk/identity guards, backup
refs, explicit lease-protected pushes, stale remote rejection and local restore.
They do not rewrite the toolkit repository or contact GitHub.
