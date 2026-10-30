# Maintaining the documentation website

The landing page is `docs/index.html`, written by hand. Its **Browse
components** link opens `docs/reference/index.html`; **Get started** opens the
getting-started guide. The component library does not replace the landing page.

Every claim on the landing page should be checkable against the code or the
guides. `scripts/test_repo.py` keeps the component, pattern and page counts in
step with `assets/catalog-data.json`, checks the live preview slots against
Catalog metadata, and rejects palette names/settings the kit does not have.

## Landing page rendering

The landing page mounts the original Catalog dashboard, counter and plan card
through `examples/web/src/lib.rs`. That example includes `examples/catalog`
directly, with native OS services gated out. Rust remains responsible for
component layout, paint, state and callbacks. Do not add HTML lookalikes or
component screenshots. `docs/live/catalog-previews.js` is only the lifecycle,
appearance and bounded resource-pool adapter. See [the browser guide](web-prototype.md).

The HTML display font is local WOFF2; the canvas uses licensed bundled Inter,
not redistributed Apple fonts. `images/og-card.jpg` stays a static social-link
preview, not a component demo. See [third-party notices](third-party.md).

## What is generated

- `docs/reference/`: every native Catalog page, with section previews, the
  Catalog's Rust usage excerpts, API notes and commands to run the native demo.
- `docs/guides/`: HTML versions of the main Markdown guides and the experimental iOS/web guides. Edit the
  Markdown, not the generated HTML.
- `docs/images/components/`: historic settled native software exports, retained
  for regression comparison but **not included in the published site**. The
  reference section previews are direct Rust canvas mounts, not these PNGs.
- `docs/assets/catalog-data.json`: source-backed Catalog metadata.
- `docs/site-manifest.json`: the explicit generated publishing allowlist.

The Catalog usage strings are excerpts, not all standalone programs. Page
notes link to the complete example source and the runnable first-window guide.
Browser and native interaction tests are separate acceptance gates.
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

Review the generated HTML, live slots and allowlist. The generator rewrites its
listed outputs, but never deletes files. If a Catalog page/section is removed,
review and remove its old generated files explicitly; they are not published
unless still listed. The Rust regression test checks published metadata
against all current Catalog pages and sections, so stale usage/API text fails.

## Preview the exact publishing artifact

```sh
python3 scripts/build_web.py
python3 scripts/build_docs_site.py work/site-preview-new --web-assets docs/live/pkg
python3 -m http.server 8000 --bind 127.0.0.1 --directory work/site-preview-new
```

Only manifest-approved HTML and Catalog JSON are added to fixed base assets.
The explicit web-assets option stages just the two expected bindgen outputs
and the canonical adapter. Missing/linked/escaping files are rejected. Arbitrary
docs, logs, source files and credentials are not published. Markdown raw HTML
is escaped; CSP permits only same-origin modules and WASM compilation on
landing/reference pages. Guides remain script-free. No analytics or runtime
CDN requests are added.

Check desktop and narrow layouts, dark mode, mobile navigation, source
disclosures, keyboard focus and local links before publishing. GitHub Pages
publication remains the separate manual step in the
[public-release checklist](public-release.md).


## Live Rust previews

Run `python3 scripts/build_web.py` using the versioned tools in
[the web guide](web-prototype.md). Do not commit generated JS/WASM. All 123
reference slots reuse original Catalog section declarations. Four sections
covering native lifecycle/files/OS services show explicit capability notices;
background threads and external navigation remain native-only actions.

Directory thumbnails are fresh software runtime draws, released after their
first frame. Detailed pages retain interactive Rust state while offscreen but
release render resources. One shared module, at most eight active previews and
two WebGPU surfaces prevent one device/instance per card. Verify every section
with `scripts/test_catalog_web.cjs`, as well as `scripts/test_web.cjs` for the
small starter/host-input fixture. Native compatibility is a separate gate.

Pages deployment requires **GitHub Actions**, not raw branch `/docs` publishing:
compiled WASM/JS build products are ignored by Git. After merging, select Actions
in **Settings → Pages → Build and deployment**, then manually run the
**Documentation site** workflow on `main`. See the public-release checklist.
