# Maintaining the documentation website

The landing page is `docs/index.html`. Its **Explore components** link opens
`docs/reference/index.html`; **Start building** opens the getting-started guide.
The component library does not replace the landing page.

## What is generated

- `docs/reference/`: every native Catalog page, with section previews, the
  Catalog's Rust usage excerpts, API notes and commands to run the native demo.
- `docs/guides/`: HTML versions of the eight main Markdown guides. Edit the
  Markdown, not the generated HTML.
- `docs/images/components/`: settled, light/Violet **software-rendered** snapshots
  from the actual Rust builder trees. They are not HTML recreations, GPU
  screenshots, native-input acceptance evidence or desktop-blur captures.
  Long/scrollable demos remain viewport captures. Click an image to enlarge it.
- `docs/assets/catalog-data.json`: source-backed Catalog metadata.
- `docs/site-manifest.json`: the explicit generated publishing allowlist.

The Catalog usage strings are excerpts, not all standalone programs. Page
notes link to the complete example source and the runnable first-window guide.
Open popups and hover/keyboard states still need native testing in the Catalog.
Snapshots use installed fonts; no Apple font binary is distributed. The dynamic
date demos use the date on which the exporter runs.

## Regenerate after Catalog or guide changes

Run from the repository root. Use a **new** export directory each time; the
exporter deliberately refuses existing output instead of deleting it.

```sh
cargo run --locked --features native --example catalog -- \
  --export-docs work/catalog-export-new

# Maintainer-only Markdown parser, isolated from Rust and the shipped site.
# Node 22+ is required; there are no client-side JavaScript dependencies.
npm install --prefix work/docs-tooling --ignore-scripts --no-audit --no-fund \
  --no-save marked@17.0.5
NODE_PATH="$PWD/work/docs-tooling/node_modules" \
  node scripts/generate_docs.cjs work/catalog-export-new

python3 scripts/check_repo.py
python3 -m unittest discover -s scripts -p 'test_*.py'
cargo test --locked --features native --example catalog
```

Review the generated diff, images and allowlist. The generator rewrites its
listed outputs, but never deletes files. If a Catalog page/section is removed,
review and remove its old generated files explicitly; they are not published
unless still listed. The Rust regression test checks published metadata
against all current Catalog pages and sections, so stale usage/API text fails.

## Preview the exact publishing artifact

```sh
python3 scripts/build_docs_site.py work/site-preview-new
python3 -m http.server 8000 --bind 127.0.0.1 --directory work/site-preview-new
```

Only manifest-approved HTML, preview PNGs and the Catalog JSON are added to the
fixed base assets. Missing/linked/escaping files are rejected. Arbitrary docs,
logs, source files or credentials are not published. Markdown raw HTML is
escaped by the generator; the output keeps a restrictive CSP, local fonts and
images, no scripts, analytics or runtime CDN requests.

Check desktop and narrow layouts, dark mode, mobile navigation, source
disclosures, keyboard focus and local links before publishing. GitHub Pages
publication remains the separate manual step in the
[public-release checklist](public-release.md).
