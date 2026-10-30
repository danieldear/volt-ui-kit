# Volt UI Kit guides

Start at the [HTML landing page](index.html), browse the
[component reference](reference/index.html), or read the guides below.
The HTML site includes every Catalog page and the guides below; Markdown stays
the editable source for the guides. Component previews now render the original
Rust Catalog builders in browser canvases, not image files. See the
[web host setup](web-prototype.md) for building the runtime locally.

For the complete native demo, run `./scripts/run_catalog.sh` from the repository
root. It handles build features and macOS packaging; direct Cargo commands in
profiling/snapshot guides intentionally exercise a narrower path. See the
[launch modes](../examples/README.md#catalog-launch-modes).

The current repository version is **0.5.0**; it is not a crates.io publication.
Installation instructions remain Git/path based until publication is verified.

1. [Getting started](getting-started.md): install, features, your first window, templates and platform support.
2. [Building interfaces](building-uis.md): state and callbacks, keys, layout, responsive design, layers and scrolling.
3. [Components](components.md): buttons, navigation, fields, forms, menus, lists, tables, charts and icons.
4. [Styling and themes](styling-and-themes.md): colours, custom themes, fonts, effects, animation, images and vector drawing.
5. [Windows and rendering](windows-and-rendering.md): title bars, transparency and blur, GPU and software drawing, diagnostics.
6. [Application services](app-services.md): background work, commands and shortcuts, close guards, files, menus, notifications, the dev loop, packaging and notarized releases.
7. [JSON interfaces](json-ui.md): the optional file-based frontend with live reload.
8. [Architecture](architecture.md): layers, the frame pipeline, accessibility, security limits and dependencies.

Experimental platform work: [reusable iOS host and Rust-first project template](ios-prototype.md), including Android direction and explicit Simulator/device limits.

Also see the [examples](../examples/README.md), [contributing](../CONTRIBUTING.md),
the [roadmap](../ROADMAP.md) and the [changelog](../CHANGELOG.md).

Rust examples in these guides are covered by `cargo test --all-features --doc`.
Some are compile-only or explicitly ignored; a passing doctest run is not
proof that every example was exercised in a native window.

Maintainers: [public-release checklist](public-release.md). Distributors: [third-party notes](third-party.md).

Repository maintenance: [low-disk Git identity correction](git-identity.md).

Website maintainers: [regenerate the component previews and guides](site-maintenance.md).

- [WebAssembly browser prototype](web-prototype.md): live Rust component demos, build steps and explicit limitations.
