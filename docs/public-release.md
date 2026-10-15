# Public-release checklist

This is an operational checklist, not a certification. Preparing files does
not change GitHub visibility, protect branches or publish a Pages site.

## Prepared in this change

- `docs/index.html`: responsive static landing page with local screenshots,
  a self-hosted OFL display face, system interface fonts, no JavaScript/trackers/CDN
  assets and a restrictive content policy.
- `Repository security`: hygiene/link checks, full reachable-history Gitleaks
  scanning with redacted output and a checksum-pinned scanner, and a weekly
  RustSec dependency audit. No credentials or reports are uploaded as artifacts.
- Dependabot for Cargo and Actions; SHA-pinned Actions with read-only tokens
  and checkout credentials not persisted. There is no privileged PR execution.
- Issue/PR templates, maintainer CODEOWNERS, security policy and license notes.
- `.github/main-protection.json` and `scripts/configure_github.py` describe and
  apply main protection. They never change visibility or publish a site.

## Live settings checked on 2026-10-15

| Setting | Verified state |
| --- | --- |
| Repository visibility | Private; unchanged by this work |
| Default branch | `main` |
| Actions default token / PR approval | Read-only / cannot approve PRs |
| Actions SHA pinning | Required by repository policy |
| Dependency vulnerability alerts | Enabled |
| Delete merged branches | Enabled |
| Branch protection / rulesets | GitHub API returns 403: upgrade the plan or make repository public |
| Private vulnerability reporting | Not available in this private repository (404) |
| GitHub Pages | Not configured (404); not published |
| Latest remote CI | Blocked by GitHub account billing/spending limits; not a passing run |

These settings can drift; inspect them again before launch.

## Before changing visibility

1. Review and merge the preparation PR. Resolve CI billing restrictions and
   get successful `UI Kit CI` and `Repository security` runs. Never describe a
   configured workflow as a passing check.
2. Run `cargo audit` and `gitleaks git --log-opts="--all" --redact=100 .` after
   fetching all intended public branches and tags. Scans cover reachable local
   refs, not every hidden PR ref, unreachable object, attachment or release asset.
3. Review all retained branches/tags, historical files, commit author emails,
   PR/issue text, Actions logs/artifacts and screenshots for personal or private
   information. Git history contains personal author metadata; the owner must
   accept its disclosure. No history has been rewritten. A secret scan is not
   a personal-data audit. Inspect any old workflows before enabling fork PRs.
4. Rotate any exposed credential before considering a coordinated history
   cleanup. Removing a current file or adding `.gitignore` does not remove history.
5. Review [third-party obligations](third-party.md), [SECURITY](../SECURITY.md),
   maintainer ownership, release artifacts and account 2FA/recovery. Account
   MFA and collaborators must be reviewed by the owner; tooling cannot attest them.
6. Get explicit owner approval for changing visibility. GitHub cannot reliably
   undo public clones or forks. This change does **not** make the repo public.

## Protect main

GitHub Free supports protected branches for public repositories; private
repositories require an eligible plan. Upgrade first if protection must be
active before any public exposure. Otherwise, after an approved visibility
change, immediately run and verify:

```sh
# Read-only inspection:
python3 scripts/configure_github.py
# Apply after the required checks have successful runs:
python3 scripts/configure_github.py --apply --reviews 0
```

The prepared solo-maintainer policy requires a PR, up-to-date passing checks,
resolved conversations and administrator enforcement; force pushes and deletion
are disabled. Zero mandatory outside approvals allows the sole maintainer to
merge their own PR after checks. Use `--reviews 1` for a team; this also requires
CODEOWNER approval and can block a solo maintainer. Do not use admin bypasses
or remove required checks to work around failed CI.

Required checks are the three `validate (<os>-latest)` matrix jobs plus
`Repo hygiene`, `Secret scan` and `Dependency audit`. Verify those exact check
names exist on the PR before enabling the policy. The helper preserves any
existing Actions allowlist, enables vulnerability alerts, and (when public)
enables private vulnerability reporting, secret scanning and push protection.
It fails visibly when a setting is unavailable; rerun/inspect after partial
failure. API credentials need repository administration rights. Treat policy
changes as reviewed operations; the helper replaces the main-branch policy.

Verify in GitHub Settings that all settings took effect. A committed JSON file
is not enforced protection. GitHub can still permit some authorized secret
push bypasses; review any bypass notices.

## Publish the documentation

Do this **only after the repo is public and the site contents are approved**:

1. In Settings → Pages, select **GitHub Actions** as the build/deployment source.
2. Restrict the `github-pages` environment to `main` before dispatching.
3. Run the **Documentation site** workflow manually on `main`. It will refuse
   private repositories and other branches. Only the base assets and reviewed HTML/PNG/JSON paths in
   `docs/site-manifest.json` are staged; logs, Rust sources, security reports and arbitrary `docs/` files are
   not included. The job has Pages/OIDC write privileges only at deployment.
4. Verify the resulting URL and links before adding it as the repository
   homepage. The expected project URL is `https://danieldear.github.io/volt-ui-kit/`,
   but it is **not a live-site claim** until deployment succeeds.

Preview locally without publishing:

```sh
python3 scripts/check_repo.py
python3 -m unittest discover -s scripts -p 'test_*.py'
python3 -m http.server 8000 --bind 127.0.0.1 --directory docs
```

The landing page links to Markdown guides on GitHub, not an HTML conversion of
each guide. A generated API/docs portal can follow independently.

## Current assessment and limits

The initial 2026-10-15 history and clean tracked-tree scans found no credential
matches. `cargo audit` found no known vulnerabilities, with unmaintained text
stack advisories `RUSTSEC-2026-0206` / `RUSTSEC-2026-0192`. Follow up on the native
text dependencies without casually changing validated GPU/text pins or the
Rust 1.89 minimum. These results must be refreshed after further changes.

Code review focused on document/asset limits, file/packaging paths, modal input
scope, dev tooling, dependency use and repository automation. A dev-state
temporary-directory hardening fix is included with regression tests. This was
not a full independent penetration test or fuzzing campaign. Native runtime,
accessibility, cross-platform and real Volt acceptance remain separate gates.

## GitHub reference documentation

- [Protected branches and plan availability](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches)
- [Secure use of GitHub Actions](https://docs.github.com/en/actions/reference/security/secure-use)
- [Repository security quickstart](https://docs.github.com/en/code-security/getting-started/quickstart-for-securing-your-repository)

Reviewed on 2026-10-15; GitHub availability and plan requirements can change.
