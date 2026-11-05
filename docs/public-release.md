# Public-release checklist

This is an operational checklist, not a security certification. A merged PR,
package version or Git tag does not publish a crate. Repository settings and
registry availability can change; verify them again before each release.

## Verified snapshot: 2026-10-17

| Setting | Verified state |
| --- | --- |
| Repository visibility / default branch | Public / `main` |
| Main protection | Enforced for administrators; PRs, up-to-date checks and resolved conversations required |
| Outside approvals | Zero required, for the solo maintainer |
| Force pushes / branch deletion | Disabled on `main` |
| Required checks | Three OS `validate` jobs, `Repo hygiene`, `Secret scan`, `Dependency audit` |
| Merged PR #22 checks | All six succeeded |
| Actions default token / PR approval | Read-only / cannot approve PRs |
| Actions SHA pinning | Required by repository policy |
| Secret scanning / push protection | Enabled / enabled |
| Automatic Dependabot security updates | Disabled in repository settings; scheduled dependency PRs are configured separately |
| Delete merged branches | Enabled |
| GitHub Pages | API reports `built`, with legacy deployment source; not the manual Actions source described below |
| crates.io | No `volt-ui-kit` entry returned by the registry API; 0.5.0 is prepared, not published |

The Pages URL is <https://danieldear.github.io/volt-ui-kit/>. The API build state
alone does not establish which revision is deployed. Review the actual site
before announcing it or changing the repository homepage.

## Before a release

1. Merge the reviewed release PR with all required checks passing. Do not
   weaken branch protection to work around failures.
2. Fetch the intended branches and tags, then run `cargo audit` and
   `gitleaks git --log-opts="--all" --redact=100 .`. Scans cover reachable local
   refs, not every hidden PR ref, unreachable object, attachment or release asset.
3. Review retained history, author emails, PR/issue text, Actions logs/artifacts
   and screenshots for private information. A secret scan is not a personal-data
   audit. Review historical workflows before enabling fork PRs.
4. Rotate exposed credentials before a coordinated history cleanup. Deleting a
   current file or adding `.gitignore` does not remove historical disclosure.
5. Review [third-party obligations](third-party.md), [SECURITY](../SECURITY.md),
   ownership, release artifacts and account 2FA/recovery. The owner must check
   account MFA, collaborators and registry access; local checks cannot attest them.
6. Keep version references, [changelog](../CHANGELOG.md), README, guides and
   generated HTML synchronized. Follow [site maintenance](site-maintenance.md)
   and [contribution checks](../CONTRIBUTING.md#checks).

## Protect main

Inspect the live policy without changing it:

```sh
python3 scripts/configure_github.py
```

The solo-maintainer policy requires a PR, up-to-date passing checks, resolved
conversations and administrator enforcement. Zero mandatory outside approvals
allows the sole maintainer to merge after checks. Force pushes and deletion are
blocked. The required names are `validate (ubuntu-latest)`,
`validate (macos-latest)`, `validate (windows-latest)`, `Repo hygiene`,
`Secret scan` and `Dependency audit`.

The reviewed policy lives in `.github/main-protection.json`. If an owner-approved
policy change is needed, `python3 scripts/configure_github.py --apply --reviews 0`
replaces the main-branch policy and applies supported security settings. It needs
repository administration rights and can fail partially; inspect the live state
afterwards. Use `--reviews 1` only when another approver is available. A committed
JSON file is not enforced protection. Never use an admin bypass to hide failed CI.

## Publish the documentation

The repository contains a landing page, 50 component-reference pages and HTML
guides. The published component previews run the original Rust Catalog in WASM;
prose and source remain readable without JavaScript. Fonts and runtime assets
are local; no runtime CDN or tracker is added. Native-only OS services display
capability notices, not simulated permissions or desktop blur.

**Deployment source must be GitHub Actions**, not “Deploy from a branch /docs”.
The generated `docs/live/pkg` binaries are deliberately ignored by Git and must
be compiled before publishing. After merging the browser-host changes:

1. Open **Settings → Pages → Build and deployment → Source → GitHub Actions**.
2. Restrict the `github-pages` deployment environment to `main`.
3. Open **Actions → Documentation site → Run workflow**, selecting `main`.
4. Wait for both build and deployment to succeed, then verify the deployed
   revision, links and interactive previews before announcing the URL.
   Do not count a successful old branch deployment as WASM acceptance.

The reviewed workflow installs the pinned bindgen CLI, compiles the Rust example,
and stages only approved HTML/CSS/fonts/licenses plus the expected WASM/JS
outputs and canonical adapters. Publication remains a manual maintainer action.

The workflow refuses private repositories and branches other than `main`. Only
base assets and reviewed paths in `docs/site-manifest.json` are staged, not
arbitrary files under `docs/`.

Preview without publishing:

```sh
python3 scripts/check_repo.py
python3 -m unittest discover -s scripts -p 'test_*.py'
python3 scripts/build_web.py  # requires wasm32 target and wasm-bindgen-cli 0.2.129
python3 -m http.server 8000 --bind 127.0.0.1 --directory docs
```

## Publish the crate

The command is **`cargo publish`**, not `cargo push`. A dry run packages and
verifies the archive without uploading it. Use a clean, reviewed release checkout
with the same version in Cargo.toml, Cargo.lock and release documentation:

```sh
git switch main
git pull --ff-only
git status --short                 # must be clean; inspect rather than discard changes
cargo package --locked --list     # review exactly what will be uploaded
cargo publish --dry-run --locked
cargo publish --dry-run --locked --all-features
```

Run the checks in CONTRIBUTING as well. Default-feature verification alone does
not test every optional adapter, platform or GPU runtime. The source-checkout
Catalog launcher and maintainer scripts are deliberately excluded from the crate;
consumers use the library and installed CLI, or clone the repository for the full
demo. Use Git/path installation instructions until registry publication succeeds.

Before the first upload, the maintainer must verify their crates.io account,
verified email, authentication and name availability. A registry lookup or dry
run does not reserve the name or validate upload permissions. Keep tokens out of
chat, source files and shell history; use Cargo's supported credential provider.

Only after explicit release approval and successful merged CI:

```sh
cargo publish --locked
```

This is a separate, generally irreversible upload: the same version cannot be
overwritten. Confirm the registry shows the expected version and archive before
announcing it, tagging the published release or replacing Git instructions with
registry installation. Never describe a dry run as a published release.

See [Cargo publish](https://doc.rust-lang.org/cargo/commands/cargo-publish.html)
and [the publishing guide](https://doc.rust-lang.org/cargo/reference/publishing.html).

## Security assessment and limits

The 2026-10-17 `cargo audit` run reported **zero known vulnerabilities** and seven
unmaintained-package warnings: `RUSTSEC-2026-0206` (rustybuzz),
`RUSTSEC-2026-0192` (three ttf-parser versions), and `RUSTSEC-2026-0370`,
`RUSTSEC-2026-0371`, `RUSTSEC-2026-0372` (zvariant, zbus_names, zvariant_derive).
Track these separately from exploitable-vulnerability findings. Refresh the audit
before upload; do not casually change validated GPU/text pins or Rust 1.89 support.

Passing tests and scanners are not a full penetration test or fuzzing campaign.
Native runtime, accessibility, other operating systems and real Volt consumer
acceptance remain distinct gates. The release checklist does not assert that all
of those acceptance paths were rerun for a documentation-only change.

## GitHub reference documentation

- [Protected branches](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches)
- [Secure use of GitHub Actions](https://docs.github.com/en/actions/reference/security/secure-use)
- [Repository security quickstart](https://docs.github.com/en/code-security/getting-started/quickstart-for-securing-your-repository)
