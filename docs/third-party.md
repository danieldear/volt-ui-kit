# Third-party code and assets

The toolkit's own source is MIT licensed; retain [LICENSE](../LICENSE) when
redistributing it. Dependencies keep their own licenses. This page is not a
replacement for the notices required by those licenses, nor legal advice.

## Rust dependencies

`Cargo.lock` records the reviewed dependency graph; optional native and GPU
features add more dependencies than the default drawing layer. Use
`cargo metadata --locked --all-features --format-version 1` to inspect exact
packages and license expressions for a particular checkout. Choose permitted
alternatives in dual-licensed packages and retain their required notices.

The 2026-10-15 metadata review found predominantly MIT/Apache/BSD-style
licenses. `option-ext` 0.2.0 declares MPL-2.0 (file-level obligations); packages
with GPL/LGPL alternatives also offer permissive alternatives. This is not a
license-compliance certification. Binary distributors must collect and ship
appropriate notices for their actual feature/platform graph, including native
libraries and any changes to third-party files.

## Fonts and images

No Apple font binaries are included. SF and SF Mono refer to fonts available
on supported systems or explicitly supplied by the application developer;
other platforms may use fallbacks. Do not copy Apple fonts into an app or web
bundle without verifying the applicable license. The documentation website
uses system fonts and makes no font downloads.

The images in `docs/images/` are screenshots of synthetic Catalog content.
They illustrate native output, not customer data or an embedded web demo.
The site's bolt favicon is repository-local SVG artwork. Apps remain
responsible for rights to their own images, icons and custom fonts.
