# Release readiness checklist

This is an operational checklist, not a security certification. A merged PR,
package version or Git tag does not publish a crate. Repository settings and
registry availability can change; verify them again before each release.

Use this checklist for release engineering. Use the separate
[announcements and live projects checklist](announcements-and-projects.md) for
community introductions, editorial submissions and adoption examples.

## Verified snapshot: 2026-11-12

This integration review covers `feat/chrome-app-integration`, based on code
revision `da03454`, plus the security fixes in its integration PR. It is not a
published release or a claim that these changes have already reached `main`.
Refresh the evidence on the final merged release revision.

| Setting | Verified state |
| --- | --- |
| Repository visibility / default branch | Public / `main` (`c3a125e` at review time) |
| Main protection | Administrator enforcement, PRs, up-to-date checks and resolved conversations required |
| Outside approvals | Zero required, for the solo maintainer |
| Force pushes / branch deletion | Disabled on `main` |
| Required checks | Three OS `validate` jobs, `Repo hygiene`, `Secret scan`, `Dependency audit` |
| PR #44 | Merged; subsequent chrome/Linux services changes require a separate integration PR |
| Secret scanning / push protection / private reporting | Enabled / enabled / enabled |
| Automatic Dependabot security updates | Disabled; scheduled dependency PRs exist and require review |
| GitHub Pages | Actions deployment source; the final integration revision must be deployed after merge |
| crates.io | Registry API returned no `volt-ui-kit` entry; repository version 0.6.0 is not a publication |

The Pages URL is <https://danieldear.github.io/volt-ui-kit/>. Check the actual
site and deployed revision before an announcement; an old successful deployment
is not acceptance of this candidate.

## Verified candidate baseline

Checked items are scoped evidence from this review, not final frozen-main gates,
physical-device acceptance, a security certification or Windows UI acceptance.

- [x] Local all-feature/all-target tests: 1,053 passed; 32 explicitly ignored.
- [x] Local doctests: 62 passed; seven explicitly ignored.
- [x] macOS all-target Clippy and Rust 1.89 all-target checks pass; Linux-target
  library Clippy/MSRV and WASM/mobile host build checks pass.
- [x] All eight exact copied Catalog programs and ten desktop starter variants
  compile independently. This is not proof of a clean public-registry install.
- [x] Volt consumer fixture: 255 compatibility tests passed; two ignored.
- [x] Reachable-history secret scan including merge diffs: 369 commits scanned,
  no leaks found; the previous non-merge scan covered 290. Final
  integration refs and the working diff must be rescanned before release.
- [x] Dependency audit after targeted lock updates: zero vulnerability-category,
  unsoundness or yanked-package entries; seven maintenance warnings remain visible.
  Their disposition is documented below, not suppressed in CI.
- [x] Rust dependency license metadata reviewed (404 resolved packages): no
  missing license expressions. MPL-2.0 obligations and permissive alternatives
  still require distributor notices; this is not a legal certification.
- [x] Live main protection, secret scanning/push protection and private
  vulnerability reporting checked without changing repository settings.
- [x] User-reported Linux desktop services acceptance, including notification
  delivery/removal, file dialogs and tray commands. Prior KDE/GNOME/COSMIC checks
  and X11 opaque Custom rendering are recorded as user acceptance, not automated
  physical-device proof. X11 transparency/blur remains unsupported/pending.
- [x] Linux VM previously passed the Catalog/library suites and X11/Wayland
  startup checks at `da03454`; the new portal special-file regression also passed
  Ubuntu CI. Full integration CI remains required after correcting stale
  platform assertions and the current-stable Clippy check.
- [x] Record a candidate performance baseline on Apple M4 Pro / 24 GiB /
  macOS 27.0.1: native idle CPU/RSS, Metal presentation traces, synthetic button
  actions and Table scroll frames at 100%, 125% and 200% scale. GPU headless
  timings include readback, not on-screen latency or app-attributed GPU/power.
  Raw logs/measurements stay in ignored local QA artifacts; Windows/Linux hardware
  performance and long-running memory/power acceptance remain separate.
  The focused animated Introduction is not static idle: its intentional ambient
  effects measured about 40.9% of one CPU core at roughly 118 frames/s on the
  120 Hz display. Do not substitute the quiet static-Button measurements for it.

### Dependency review decisions

- `async-executor` 1.14.0 was yanked; [upstream 1.14.1](https://github.com/smol-rs/async-executor/blob/master/CHANGELOG.md)
  fixes its executor-teardown use-after-free. The integration lock selects 1.14.1, including the Linux
  accessibility/D-Bus dependency graph.
- Replace yanked `clipboard-win` 5.4.1 with non-yanked 5.4.3. The registry did not
  provide a yank reason; do not invent one. Windows runtime acceptance is separate.
- Root, shared mobile and iOS example locks use Tokio 1.53.3 for
  [RUSTSEC-2026-0386](https://rustsec.org/advisories/RUSTSEC-2026-0386.html).
  Reachability was through the unstable Emscripten/Tokio configuration of
  wasm-bindgen-futures, not the standard desktop host; patch it rather than
  globally ignoring the advisory. Web locks already select patched 1.54.1.
- Seven maintenance warnings remain: rustybuzz, three ttf-parser versions,
  zbus_names, zvariant and zvariant_derive. They are maintenance risk, not proof
  of an exploitable vulnerability. Retain the validated text/GPU/native API pins
  during this scoped integration; track upstream replacements and require a
  maintainer decision before any production-readiness claim.

## Before a release

All unchecked items remain pending. Record the tested revision, date, platform
and result when completing a gate. A documented exception needs a reason and
an owner; never turn a failed check into a checked box by hiding it.

### 1. Integrate and freeze the release candidate

- [x] PR #44 merged as `c3a125e` on 2026-11-11 with all nine checks passing,
  including browser tests. Copied locks and the pinned browser runtime are fixed;
  no tests were disabled and main protection was not weakened.
- [ ] Merge subsequent reviewed candidate changes with all applicable CI passing
  and review conversations resolved. Do not bypass checks or branch protection.
- [ ] Choose the release version and scope; fold the appropriate Unreleased
  entries into dated release notes, documenting breaking API changes.
- [ ] Freeze a clean, merged `main` revision for all final checks and artifacts.
- [ ] Verify intended changes are included, with no private files, unrelated
  worktree changes or coauthor trailers in release commits.

### 2. Complete security and dependency review

- [x] Review/update locked yanked `async-executor` and `clipboard-win` packages;
  upstream executor fix and unknown clipboard yank reason are recorded above.
- [x] Patch Tokio advisory RUSTSEC-2026-0386 in the affected root/mobile/iOS locks;
  retain the target-reachability explanation and rerun the final release audit.
- [x] Record maintenance-warning decisions without changing validated GPU/text
  pins or dropping Rust 1.89 compatibility. Final production-risk acceptance
  remains a maintainer decision, not an automated approval.
- [ ] Rerun dependency and redacted history secret scans on final refs. Scans
  do not cover every hidden PR ref, unreachable object, attachment or asset.
- [ ] Review author emails, history, issues/PRs, logs, screenshots and release
  assets for private information; rotate any exposed credentials first.
- [ ] Review licenses and [third-party obligations](third-party.md), including
  bundled fonts, notices and native libraries for the actual distribution.
- [ ] Owner verifies GitHub/crates.io account MFA, recovery, collaborators and
  registry permissions; no credentials belong in chat, source or shell history.

### 3. Verify applications and supported platforms

- [ ] Rerun the [contribution checks](../CONTRIBUTING.md#checks) on the frozen
  revision, including copied samples and warning-free documentation builds.
- [ ] Create a fresh desktop project using `cargo volt new`, following only the
  documented instructions; run it outside the toolkit checkout.
- [ ] Test a fresh consumer against the packaged crate, including `native`,
  `native-gpu` and intended desktop adapters; retain Volt compatibility coverage.
- [ ] Verify navigation, editing/IME, scrolling, sorting/selection, overlays,
  keyboard focus and accessibility in native windows, not just headless renders.
- [ ] Test Linux fonts, text/border smoothness, corners and inline controls at
  100% and 125% scaling; cover KDE and the other desktops we claim to support.
- [ ] Verify blur capabilities/fallbacks per compositor; do not promise blur
  where the desktop does not support it.
- [ ] Validate Windows native behavior for any supported claims. If Linux or
  Windows acceptance remains incomplete, label those paths experimental and
  list known issues rather than claiming production parity.
- [ ] Record idle CPU, representative scrolling/frame cost and memory with
  hardware, scale, build mode and renderer; separate measurements from estimates.
- [ ] Profile and accept visible decorative-animation cost separately from
  static idle; consider developer-controlled motion budgets and offscreen
  suspension without silently changing the approved design or all animation rates.
- [ ] Keep web, iOS, Android and maps prototype limitations explicit; confirm
  which hosts/templates require a repository checkout or unpublished subcrate.

#### Mobile application scaffolding

These gates apply to advertised mobile readiness. Incomplete mobile work must
remain explicitly experimental; it need not block a scoped desktop release.

- [ ] Extend the existing iOS/Android generators rather than duplicating their
  shared native host; offer distinct minimal, navigation, tabs and settings
  presets while preserving desktop defaults and Volt compatibility.
- [ ] Generate a usable, customizable header/body/footer composition. Header,
  footer and bottom navigation are optional; do not force another title bar or
  a fixed application design on developers.
- [ ] Verify safe areas, system bars, keyboard avoidance, scrolling, rotation,
  page selection and platform Back behavior in the generated applications.
- [ ] Generate app identity, bundle/package IDs, icons, theme defaults and
  appropriate native project configuration with clear signing instructions.
- [ ] Add explicit capability/permission choices with no unnecessary permissions
  or startup prompts by default. Platform declarations alone do not implement
  native capabilities or request permission at runtime.
- [ ] For each supported permission, implement/check native status, contextual
  requests and granted/denied/revoked/unavailable results. Unsupported choices
  must be reported, not silently scaffolded as working features.
- [ ] Provide prerequisite diagnostics and documented build/run paths; keep
  signing, private credentials and personal devices developer-owned.
- [ ] Validate the complete generate/build/run/customize workflow on iOS and
  Android, including generated navigation, editable-form and scroll tests.
- [ ] Test physical-device lifecycle/input/rendering behavior before making
  production claims; publish known limitations and emulator/device coverage.

### 4. Finalize documentation and the live Catalog

- [ ] Synchronize manifests/lockfiles, README, changelog, installation commands,
  guides, platform matrix and generated HTML with the chosen release version.
- [ ] Refresh this snapshot/security assessment and remove completed or stale
  roadmap entries; do not describe prepared code as registry-published code.
- [ ] Review public API documentation and migration notes for new/breaking APIs.
- [ ] Ship version-matched agent-readable references and the shared Volt skill;
  verify generated examples, public Markdown links and `llms.txt` discovery.
- [ ] Validate the Claude Code and Codex plugin packages against the released
  toolkit, including clean installation and recorded agent-driven application
  tests. Follow the [agent delivery checklist](announcements-and-projects.md#3-support-coding-agents-with-skills-and-plugins).
- [ ] Keep agent/plugin installation optional, permission-preserving and separate
  from registry publication or public plugin-directory submission approval.
- [ ] Regenerate and validate docs using [site maintenance](site-maintenance.md).
- [ ] Deploy the approved merged revision through GitHub Pages Actions.
- [ ] Verify deployed revision, JS/WASM MIME types, links and interactive
  components, including keyboard/input behavior, in supported browsers.

### 5. Approve and publish

- [ ] Owner verifies crate-name availability, verified registry email and Cargo
  authentication. A dry run does not reserve the name or prove upload rights.
- [ ] Review `cargo package --locked --list`; exclude private/local artifacts.
- [ ] Rerun both publication dry runs below from the clean release revision.
- [ ] Obtain explicit approval for the registry upload, tag and GitHub release.
- [ ] Publish only the approved crate/version; verify registry archive and
  docs.rs builds, then test installation in a new consumer project.
- [ ] Tag the exact approved source revision and create matching release notes;
  verify provenance/checksums and signing where distributing native binaries.
- [ ] Update publication status/install instructions only after upload succeeds.
- [ ] Complete the announcement checklist; leave an issue tracker, known-issues
  list and patch-release plan ready for external users.

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

Before these integration patches, the 2026-11-11 `cargo audit` run reported
zero entries in its vulnerability category, but **not a warning-free graph**: seven unmaintained-package warnings,
one unsoundness advisory for Tokio 1.53.2 and two yanked packages
(`async-executor` 1.14.0 and `clipboard-win` 5.4.1).

[RUSTSEC-2026-0386](https://rustsec.org/advisories/RUSTSEC-2026-0386.html)
The 2026-11-12 audit after the targeted lock updates reported zero vulnerability,
unsoundness and yanked-package entries, with the same seven maintenance warnings.

The Tokio advisory concerns `AsyncFd` I/O safety. Reachability here was through
`wasm-bindgen-futures` under `cfg(all(target_os = "emscripten",
wasm_bindgen_unstable_tokio))`; it is not selected by the checked native macOS,
Linux or `wasm32-unknown-unknown` builds. This scoped reachability finding is
not a blanket safety guarantee or permission to suppress the advisory.

Maintenance warnings remain `RUSTSEC-2026-0206` (rustybuzz),
`RUSTSEC-2026-0192` (three ttf-parser versions), and `RUSTSEC-2026-0370`,
`RUSTSEC-2026-0371`, `RUSTSEC-2026-0372` (zvariant, zbus_names, zvariant_derive).
Refresh and triage the graph before upload; preserve validated GPU/text behavior
and Rust 1.89 support when updating dependencies.

Passing tests and scanners are not a full penetration test or fuzzing campaign.
Native runtime, accessibility, other operating systems and real Volt consumer
acceptance remain distinct gates. The release checklist does not assert that all
of those acceptance paths were rerun for a documentation-only change.

## GitHub reference documentation

- [Protected branches](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches)
- [Secure use of GitHub Actions](https://docs.github.com/en/actions/reference/security/secure-use)
- [Repository security quickstart](https://docs.github.com/en/code-security/getting-started/quickstart-for-securing-your-repository)
