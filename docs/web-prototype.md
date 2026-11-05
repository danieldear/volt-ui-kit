# WebAssembly browser host

**Experimental.** The landing page and component reference now draw the
**original Catalog Rust builders and callbacks** in browser canvases, instead
of loading component screenshots. All 123 published sections have a live slot;
OS-specific lifecycle, document and desktop-service sections show an explicit
native-only capability card. These notices do not emulate file access,
notifications or tray icons. This is not a claim of full web/desktop parity.

The directory draws 49 source-backed canvas thumbnails once; open a component
page for its interactive examples, usage snippets and API notes. The landing
page also reuses the original dashboard, counter and plan-card builders.

## Architecture

```text
HTML prose, source and API notes   remain readable/searchable without WASM
  |
  +-- lazy live preview
       |
       +-- Rust application state + the existing Volt ui builders/callbacks
       +-- WebView<C>: existing Runtime + RasterPainter
       +-- BrowserHost<C>
            +-- Vello / WebGPU        async initialization, GPU drawing
            +-- tiny-skia / Canvas2D  explicit CPU drawing fallback
       +-- small DOM adapter          pointer, wheel, editable input, IME,
                                      semantic nodes, resize and lifecycle
```

The separate, unpublished `crates/volt-ui-kit-web` crate is opt-in. Existing
`native`, `native-gpu`, legacy Painter/Viewport APIs and desktop defaults keep
their paths and behavior. The browser backend does not initialize winit or
native desktop services. The shared GPU presenter gained an asynchronous
constructor; native synchronous constructors retain their existing policy.

## Build and run locally

From a checkout, with Rust 1.89 or newer and Python 3:

```sh
rustup target add wasm32-unknown-unknown
cargo install --locked wasm-bindgen-cli --version 0.2.129
python3 scripts/build_web.py
python3 -m http.server 8768 --bind 127.0.0.1 --directory docs
```

Open `http://127.0.0.1:8768/index.html` or `reference/index.html`.
The standalone `/live/` starter remains a small host regression fixture. The build helper
checks the CLI version against the locked WASM dependency; it does not install
tools, accept SDK licenses, publish anything or start a network server for you.
`WASM_BINDGEN=/path/to/wasm-bindgen` can select an existing executable.

WebGPU needs HTTPS or localhost and a compatible browser/device. The playground
tries WebGPU, then clearly reports software if initialization is unavailable.
The Renderer selector also permits explicit software mode. Vello needs compute
shaders; WebGL2 is **not** a replacement for this Vello path. A runtime GPU loss
stops the demo with a visible error; reset it or select software to recover.

## Rust application code

Screen declarations are ordinary Volt Rust UI; browser JavaScript contains no
application component layout or design. The runnable example is
[`examples/web/src/lib.rs`](../examples/web/src/lib.rs).

```rust,ignore
use volt_ui_kit::ui::prelude::*;

fn counter(app: &App) -> Element<App> {
    vstack![
        text(format!("Count: {}", app.count)),
        button("Add one").key("add")
            .on_click(|app: &mut App| app.count += 1),
    ].padding(24., Edge::All).into()
}
```

Create a `WebView::new(state, view)` and asynchronously mount it with
`BrowserHost::new(canvas, view, prefer_gpu)`. A wasm-bindgen wrapper exposes
`dispatch`, `snapshot` and `frame` to the canonical `attachVolt` adapter. The
example demonstrates that small export wrapper. A reusable `cargo volt` web
project template is not part of this first slice.

## Fonts and lifecycle

Browsers cannot inspect installed system fonts as desktop apps do. The example
registers bundled Inter regular/italic and JetBrains Mono font bytes before constructing its painter, under the
[SIL Open Font License](live/OFL.txt). SF/SF Mono are **not redistributed**.
Applications may register their own appropriately licensed fonts. The bundled
fonts do not cover every script or emoji: localized applications must register
fonts for their required glyph coverage before shipping. Unicode input/state
correctness tests are not proof of complete font coverage.

The adapter cancels listeners, observers and pending animation frames on dispose.
It pauses rendering offscreen or while the document is hidden, respects reduced
motion, and does not poll or redraw indefinitely when idle. All previews in one
document share one WASM module. The docs pool permits at most eight active
previews and two WebGPU surfaces; other active previews explicitly use software.
Offscreen interactive previews release surfaces and paint caches but retain
Rust application state. Each reference section owns an isolated Rust application state; different
sections do not share selections or edits. Directory cards release their host after the first
runtime draw, retaining canvas pixels until an appearance change requires a new
draw. They are not downloaded PNGs or independent WASM instances.

Native OS services, unrestricted filesystem access, tray icons and desktop
behind-window blur are unavailable in the browser. Browser permissions and
security boundaries are not bypassed.

## Editing and password boundaries

Public text editors synchronize DOM selection offsets (UTF-16) to Rust byte
boundaries, rejecting offsets inside surrogate pairs. Composition updates paint
through the native Runtime without changing committed application text; blur,
suspension and target removal cancel drafts. The standalone playground's
**Editor** tab demonstrates an actual `code_view` with application-owned edits,
paste, copy/cut and undo. It is a small QA document model, not a production IDE.

Password fields use a write-only `type=password` sink anchored to the painted
caret. It never receives an existing Rust password, value or selection from a
snapshot. Committed input goes into Rust; transient DOM contents are cleared on
commit, blur and disposal, and copy/cut do not export the secret. This does not
make a browser a secure enclave: typed characters necessarily enter browser
memory, JavaScript on the same origin must be trusted, and zeroization,
password-manager/autofill parity and physical IME acceptance are not claimed.
Browser adapters must not log input packets or application secrets.

## Validation and remaining acceptance

Shared host tests cover bounds, decoding, callbacks, disabled/missing targets,
password non-disclosure, suspension, scaling, scrolling and software pixels.
The browser smoke suite tests actual pointer routing, keyboard tabs, Unicode
form edits, composition commit gating, scrolling, themes, compact resize, reset,
idle-frame stability and preferred rendering. Run with Playwright 1.62.1:

```sh
# Install test tools into an ignored folder, not the application's dependencies.
npm install --prefix target/web-test-tools --no-save playwright@1.62.1
npx --prefix target/web-test-tools playwright install chromium
NODE_PATH="$PWD/target/web-test-tools/node_modules" node scripts/test_web.cjs
NODE_PATH="$PWD/target/web-test-tools/node_modules" node scripts/test_catalog_web.cjs
NODE_PATH="$PWD/target/web-test-tools/node_modules" node scripts/test_web_input.cjs
```

Set `PLAYWRIGHT_CHANNEL=chrome` to use installed Chrome. Set
`VOLT_TEST_WEBKIT=1` after installing Playwright WebKit for an additional engine
run. `VOLT_WEB_URL` changes the test server address; default is localhost:8768.
Screenshots are written under `target/web-qa/`. The Catalog suite renders every
published section, checks nonblank pixels, and exercises the original landing,
menu and context-menu callbacks plus resource-pool limits.

**Not yet production acceptance:** comprehensive Safari/Firefox/mobile-browser
coverage, real-device energy/GPU benchmarks, full assistive-technology behavior,
physical input-method candidate/keyboard acceptance, font-matched pointer and
candidate geometry across scales, advanced editor shortcut
and multi-selection parity, password-manager/autofill integration,
touch-drag/multitouch/inertial scrolling, web project scaffolding and exhaustive interaction coverage for every Catalog demo.
The basic DOM semantic bridge is not a claim of complete web accessibility.

## Publishing the documentation

The prose, source and API notes stay readable without JavaScript; component
canvases require WASM. The landing/reference pages load only the reviewed
same-origin `catalog-previews.js` module. Guides remain script-free. Their CSP
permits WASM compilation (`wasm-unsafe-eval`), not general JavaScript
`unsafe-eval`. There are no inline scripts, component iframes, runtime CDNs or
general-purpose JS manifests. Historic native PNG exports remain useful for
regression comparison but are not published as component previews.

```sh
python3 scripts/build_web.py
python3 scripts/build_docs_site.py target/pages-with-web --web-assets docs/live/pkg
```

The stager accepts exactly the two expected wasm-bindgen outputs plus the
canonical event and editing adapters. Generated WASM/JS are ignored, not committed. The manual
GitHub Pages workflow builds and stages those assets before publishing.

GitHub Pages must use **GitHub Actions** as its deployment source. Publishing
raw `/docs` from a branch will omit the ignored WASM binary and will not run
these previews. Merge the source changes, switch **Settings → Pages → Source**
to **GitHub Actions**, then manually run **Documentation site** on `main`.
The browser downloads a compiled module, not Rust source or a compiler.
