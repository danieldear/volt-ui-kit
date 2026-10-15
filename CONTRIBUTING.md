# Contributing

Thanks for helping. Issues and pull requests are welcome; for larger changes,
open an issue first so we can agree on the approach.

## Layout

```text
src/
  ui.rs, ui/          Rust builders: components, charts, icons, tokens
  declarative/        tree, validation, layout, input, focus, animation, JSON frontend
  native/             window host: GPU (gpu*.rs) and software (raster.rs) drawing,
                      accessibility, menus, files, notifications
  theme.rs            colour roles, palettes, theme JSON
  *_paint.rs, svg.rs  drawing recipes and the SVG painter (no features needed)
  bin/cargo-volt.rs   `cargo volt new | dev | bundle`
examples/             Catalog and other examples (see examples/README.md)
tests/                integration tests, one file per area
docs/                 guides; their Rust examples run as doctests
```

## Checks

Run these before opening a pull request. CI is configured to run these on
macOS, Windows and Linux; check the actual run before claiming validation.

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked
cargo test --locked --all-features --all-targets
cargo test --locked --all-features --doc
cargo +1.89.0 check --locked --all-targets --all-features
cargo check --locked --lib --target wasm32-unknown-unknown --features rust-ui
```

Some tests need a GPU or measure timing, so they are ignored by default. Run
them on a machine with a GPU when you touch rendering:

```sh
cargo test --locked --all-features --lib native::gpu -- --ignored --nocapture
SNAP=target/catalog-interaction.png cargo test --locked --all-features --example catalog -- --ignored --nocapture --test-threads=1
```

On Linux, install a font (for example `fonts-dejavu-core`) for the text tests.

## Guidelines

- **Add new components to the Catalog.** Each component gets a page with a
  live preview, its source and an API table, plus tests in `tests/`.
- **Keep the default build small.** With no features the crate depends only on
  `unicode-width`. New dependencies belong behind a feature.
- **Stay on Rust 1.89.** Don't take dependency updates that raise the minimum
  version without discussing it first.
- **No `unsafe`.** The crate forbids it; platform code goes through safe bindings.
- **Bound every input.** Sizes, counts, text lengths and caches have explicit
  limits, and invalid input is an error, not a panic.
- **Test rendering changes visually.** Use headless snapshots
  (`--snapshot out.png --size 1440x900 --scale 2`) and compare light and dark
  themes at 1x and 2x.
- **Docs describe the current behaviour.** Update the guide and the
  changelog with the code. Keep logs, screenshots and measurements from local
  testing in the ignored `work/` folder rather than in the docs.

## Performance work

Measure before and after with the same scene, scale and build:

```sh
VOLT_UI_TRACE=1 cargo run --release --features native-gpu --example catalog -- --page dashboard
cargo run --release --features native-gpu --example catalog -- --page button --bench 60 --scale 2
PROBE_PAGE=dashboard cargo test --release --all-features --example catalog interaction_cost_per_page -- --ignored --nocapture
```

Don't benchmark while something else is compiling.

## Releases

1. Move the changelog's *Unreleased* entries under the new version.
2. Bump `version` in `Cargo.toml`.
3. Run all checks above, `cargo package --locked` and `cargo audit`.
4. Tag the release.

## The Volt terminal

This library started as the interface layer of the Volt terminal, which still
vendors a copy of it. `scripts/sync_into_volt.py --volt <checkout>` refreshes
that copy reproducibly (`--check` verifies it). Never edit the vendored copy
directly.

## Public repository workflow

- Use a feature branch and a PR to `main`; do not force-push the default branch.
- Run `python3 -m unittest discover -s scripts -p 'test_*.py'` and
  `python3 scripts/check_repo.py` for documentation/tooling changes.
- Review workflow/dependency changes carefully. Actions use immutable SHAs;
  PR jobs have read-only tokens, no repository secrets, and no privileged
  `pull_request_target` execution. Never run untrusted PR code on your own Mac.
- Do not include `.env`, private keys, signing profiles, personal app state,
  tokens or unredacted logs. Screenshot only synthetic example content.
- Report vulnerabilities privately using [SECURITY.md](SECURITY.md).
- See the [publication checklist](docs/public-release.md) for protection and
  Pages settings. Never weaken failing required checks just to merge a PR.
