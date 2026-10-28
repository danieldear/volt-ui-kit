# Maintaining the documentation website

The landing page is `docs/index.html`, written by hand. Its **Browse
components** link opens `docs/reference/index.html`; **Get started** opens the
getting-started guide. The component library does not replace the landing page.

Every claim on the landing page should be checkable against the code or the
guides. `scripts/test_repo.py` keeps the component, pattern and page counts in
step with `assets/catalog-data.json`, requires every image it shows to exist and
be published, and rejects palette names and settings the kit doesn't have.

## Landing page images

The landing page shows only the Catalog. Every image is a headless render by
Volt, not a screen capture, and is labelled that way. Build the Catalog once,
then render from the repository root. These direct commands are deliberately
headless snapshot/export paths, not replacements for the full interactive
`./scripts/run_catalog.sh` launcher. No OS permission is requested by rendering:

```sh
cargo build --release --locked --features native-gpu,native-desktop --example catalog
C=target/release/examples/catalog

# The Catalog window (1440x900 at 2x), published at full and half size:
for mode in light dark; do
  $C --page dashboard --theme $mode --accent violet \
    --snapshot work/dashboard-$mode.png --size 1440x900 --scale 2
  cwebp -q 88 -m 6 -sharp_yuv work/dashboard-$mode.png \
    -o docs/images/catalog/dashboard-$mode.webp
  cwebp -q 90 -m 6 -sharp_yuv -resize 1440 0 work/dashboard-$mode.png \
    -o docs/images/catalog/dashboard-$mode-1440.webp
done

# The theme picker: the Catalog's landing page at 3x in every accent and mode,
# cropped to its plan card and its counter preview.
for mode in light dark; do for accent in zinc blue violet rose green orange; do
  $C --page introduction --theme $mode --accent $accent \
    --snapshot work/intro-$mode-$accent.png --size 1440x3700 --scale 3
  cwebp -q 90 -m 6 -sharp_yuv -crop 3233 9640 1020 1160 work/intro-$mode-$accent.png \
    -o docs/images/theming/card-$accent-$mode.webp
  cwebp -q 92 -m 6 -sharp_yuv -crop 2643 8875 1449 383 work/intro-$mode-$accent.png \
    -o docs/images/theming/counter-$accent-$mode.webp
done; done
```

The crop rectangles depend on the landing page's layout at a 1440-point-wide
window. If that page changes, find the card and the counter again; each crop's
edge must be exactly the colour behind it on the site (white or `#09090B` for
the card, `#0B0B10` for the counter's code card), so the renders sit flush.
`images/og-card.jpg` is the 1200x630 link preview.

The display font ships as WOFF2, converted from the upstream TrueType file
without changes to its glyphs (see [third-party notices](third-party.md)).

## What is generated

- `docs/reference/`: every native Catalog page, with section previews, the
  Catalog's Rust usage excerpts, API notes and commands to run the native demo.
- `docs/guides/`: HTML versions of the main Markdown guides and the experimental iOS guide. Edit the
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
