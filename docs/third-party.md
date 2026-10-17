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
uses system fonts for interface text/code and a self-hosted Bricolage Grotesque display face. There are no third-party font
requests at runtime.

The images in `docs/images/` are screenshots of synthetic Catalog content.
They illustrate native output, not customer data or an embedded web demo.
The site's bolt favicon is repository-local SVG artwork. Apps remain
responsible for rights to their own images, icons and custom fonts.

### Website display font

`docs/assets/fonts/bricolage-grotesque.woff2` is the variable Bricolage
Grotesque font by The Bricolage Grotesque Project Authors, under the SIL Open
Font License 1.1, converted from the upstream TrueType file to WOFF2 with
fontTools 4.60.1 (compression only: glyphs, axes and names are unchanged). Its full [license](assets/fonts/OFL.txt) ships with the site.
Source: [Google Fonts](https://github.com/google/fonts/tree/94301472b4d809299b4775bc6502592b83dd6934/ofl/bricolagegrotesque),
revision `94301472b4d809299b4775bc6502592b83dd6934`, retrieved 2026-10-15.
The upstream TrueType file is 408,496 bytes and the WOFF2 file 204,904 bytes; no
Apple font binaries are redistributed.
