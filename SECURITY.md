# Security policy

## Report privately

Do **not** disclose vulnerabilities, exploit details, credentials or sensitive
files in public issues, pull requests or discussions.

When private vulnerability reporting is enabled, use
[Report a vulnerability](https://github.com/danieldear/volt-ui-kit/security/advisories/new).
If that option is unavailable, contact the maintainer through
[their GitHub profile](https://github.com/danieldear) to arrange a private channel;
do not put the vulnerability details in a public profile comment or issue.
Private reporting must be enabled as part of the publication checklist.

Include the affected commit/version, feature flags, platform, a minimal
reproduction and expected impact. Use synthetic data. Never send passwords,
private signing keys or customer documents. Response is best effort; there is
no guaranteed response time or bug bounty.

## Supported versions

This is a pre-1.0 project. Security fixes target the current `main` branch and
latest published release, if any; older snapshots are not maintained release
lines. The crate is not yet published on crates.io. Depend on a reviewed Git
revision and monitor advisories and updates.

## Trust boundaries

- Rust callbacks, Cargo projects, build scripts, dev-loop state serializers and
  bundle configuration are **trusted application code**, not sandboxed input.
  `cargo volt dev`, builds and release commands execute local project code.
- JSON declarations are data, not executable scripts. Documents, decoded
  assets and rendering resources have explicit limits; these limits mitigate
  resource exhaustion but are not a process sandbox or a guarantee against
  every pathological input. Prefer trusted fonts and assets.
- File helpers and packaging validation do not defend against an attacker
  who controls your account, parent directories or concurrently changes the
  filesystem. See [architecture boundaries](docs/architecture.md#security-and-resource-boundaries).
- Opt-in dev-state handoffs use a fresh temporary directory, owner-only on
  Unix and inherited temporary-folder ACLs on Windows. Returning from the dev loop removes
  it; signal termination (including Ctrl-C), a crash or a forced kill can leave
  it behind. Never serialize credentials
  or sensitive documents as dev-state. Password fields are not a secret vault.
- Platform adapters and dependencies contain native FFI even though this
  crate forbids `unsafe` Rust. GPU drivers and OS permission prompts remain
  outside the toolkit's security boundary.
- Compositor blur uses a private macOS API and is opt-in; prefer the public
  Material mode for normal applications and distribution compatibility.

## Maintainer checks

Run `cargo audit`, a redacted Gitleaks history scan, normal regression tests and
repository checks before release. Maintenance warnings are tracked, not
silenced. Automated scans do not constitute an independent security audit.
GitHub security features and branch protection must be verified in the live
repository; committed configuration alone does not enable them.
