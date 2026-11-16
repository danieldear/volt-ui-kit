use super::*;
use volt_ui_kit::{
    declarative::{Bindings, Runtime},
    native::RasterPainter,
    Viewport,
};

fn mount(app: &App, size: [f32; 2]) -> Runtime {
    let (document, handlers) = view(app).build().expect("valid tree").into_parts();
    let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
    let viewport = Viewport::new(size[0], size[1], 1.).unwrap();
    runtime
        .layout(
            viewport,
            &Bindings::default(),
            &mut RasterPainter::new(viewport),
        )
        .unwrap();
    runtime
}

#[test]
fn every_screen_and_overlay_builds_in_both_themes_and_sizes() {
    let mut app = App::new();
    for mode in [ThemeMode::Dark, ThemeMode::Light] {
        app.mode = mode;
        for screen in Screen::ALL {
            app.screen = screen;
            for size in [[1320., 900.], [760., 900.]] {
                mount(&app, size);
            }
        }
    }
    app.screen = Screen::Route;
    app.reserve_open = true;
    app.nights_open = true;
    mount(&app, [1320., 900.]);
    app.reserve_open = false;
    app.reset_open = true;
    mount(&app, [1320., 900.]);
}

#[test]
fn filters_search_and_bookings_update_state() {
    let mut app = App::new();
    app.filter = Filter::Night;
    assert_eq!(app.visible_places(), vec![3, 4]);
    app.filter = Filter::All;
    app.query = "lofoten".into();
    assert_eq!(app.visible_places(), vec![0, 1]);
    app.query = "nowhere".into();
    assert!(app.visible_places().is_empty());
    let one = app.stay_price();
    app.nights = 3;
    assert_eq!(app.stay_price(), one / 2 * 3);
    app.saved[0] = true;
    app.reset();
    assert!(!app.saved[0]);
    assert_eq!(app.photos.len(), 6);
    assert!(app.toast.is_some());
}

#[test]
fn units_convert_for_display() {
    let mut app = App::new();
    assert_eq!(app.distance(890), "890 km");
    app.units = Units::Imperial;
    assert_eq!(app.distance(890), "553 mi");
    assert_eq!(app.temperature(-4), "25°F");
}

#[test]
fn pointer_and_text_input_drive_the_route_screen() {
    use volt_ui_kit::declarative::Handlers;
    let mut app = App::new();
    let viewport = Viewport::new(1320., 900., 1.).unwrap();
    let mut painter = RasterPainter::new(viewport);
    let rebuild = |app: &App,
                   runtime: &mut Runtime,
                   handlers: &mut Handlers<'static, App>,
                   painter: &mut RasterPainter| {
        let (document, next) = view(app).build().unwrap().into_parts();
        runtime
            .replace_document(document, &next, &Bindings::default())
            .unwrap();
        *handlers = next;
        runtime
            .layout(viewport, &Bindings::default(), painter)
            .unwrap();
    };
    let (document, mut handlers) = view(&app).build().unwrap().into_parts();
    let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
    runtime
        .layout(viewport, &Bindings::default(), &mut painter)
        .unwrap();
    let center = |runtime: &Runtime, id: &str| {
        let [x, y, w, h] = runtime
            .boxes()
            .iter()
            .find(|b| b.id == id)
            .unwrap_or_else(|| panic!("{id}"))
            .bounds
            .bounds();
        (x + w / 2., y + h / 2.)
    };
    // Clicking a place card selects it for the hero.
    let (x, y) = center(&runtime, "place-0");
    runtime
        .pointer_down_outcome(x, y, &mut handlers, &mut app)
        .unwrap();
    assert!(
        runtime
            .pointer_up_outcome(x, y, &mut handlers, &mut app)
            .unwrap()
            .activated
    );
    assert_eq!(app.place, 0);
    rebuild(&app, &mut runtime, &mut handlers, &mut painter);
    // Typing into the search field filters the strip.
    let (x, y) = center(&runtime, "search");
    runtime
        .pointer_down_outcome(x, y, &mut handlers, &mut app)
        .unwrap();
    runtime
        .pointer_up_outcome(x, y, &mut handlers, &mut app)
        .unwrap();
    assert!(runtime.editing_focus());
    runtime.text_input("sen", &mut handlers, &mut app).unwrap();
    assert_eq!(app.query, "sen");
    assert_eq!(app.visible_places(), vec![3]);
}

#[test]
fn wheel_scrolls_the_page_and_the_place_strip_with_a_toast_showing() {
    let mut app = App::new();
    app.notify("Saved");
    let mut runtime = mount(&app, [1320., 900.]);
    let page = |r: &Runtime| {
        r.boxes()
            .iter()
            .find(|b| b.id == "page-route")
            .unwrap()
            .scroll_offset
    };
    // Over the hero photo: bubbles from the hero layers to the page scroll view.
    assert!(runtime.scroll_at(600., 300., [0., 120.]));
    runtime
        .layout(
            Viewport::new(1320., 900., 1.).unwrap(),
            &Bindings::default(),
            &mut RasterPainter::new(Viewport::new(1320., 900., 1.).unwrap()),
        )
        .unwrap();
    assert_eq!(page(&runtime)[1], 120.);
    // Horizontal wheel over a place card scrolls the strip, not the page.
    let strip = runtime
        .boxes()
        .iter()
        .find(|b| b.id == "places")
        .unwrap()
        .bounds
        .bounds();
    assert!(runtime.scroll_at(strip[0] + 100., strip[1] + 100., [80., 0.]));
}

#[cfg(feature = "native-gpu")]
#[test]
#[ignore = "requires an actual GPU"]
fn every_screen_stays_within_the_gpu_layer_budget_at_retina_size() {
    use volt_ui_kit::native::GpuOffscreenRenderer;
    let mut gpu = GpuOffscreenRenderer::new().expect("GPU required");
    let mut app = App::new();
    let viewport = Viewport::new(2640., 1800., 2.).unwrap();
    for screen in Screen::ALL {
        app.screen = screen;
        let mut painter = RasterPainter::new(viewport);
        let (document, handlers) = view(&app).build().unwrap().into_parts();
        let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
        runtime
            .layout(viewport, &Bindings::default(), &mut painter)
            .unwrap();
        runtime.settle_animations();
        let batches = runtime.paint_ui(&mut painter, &theme(&app, ThemeMode::Dark));
        // Warm caches, then time settled frames (render + blocking readback).
        gpu.render(&mut painter, &batches, [0., 0., 0., 1.])
            .unwrap();
        let start = std::time::Instant::now();
        for _ in 0..20 {
            gpu.render(&mut painter, &batches, [0., 0., 0., 1.])
                .unwrap();
        }
        let ms = start.elapsed().as_secs_f64() * 1000. / 20.;
        let mut cpu = RasterPainter::new(viewport);
        runtime
            .layout(viewport, &Bindings::default(), &mut cpu)
            .unwrap();
        let cpu_batches = runtime.paint_ui(&mut cpu, &theme(&app, ThemeMode::Dark));
        cpu.rasterize(&cpu_batches, [0., 0., 0., 1.]).unwrap();
        let start = std::time::Instant::now();
        for _ in 0..20 {
            cpu.rasterize(&cpu_batches, [0., 0., 0., 1.]).unwrap();
        }
        let sw = start.elapsed().as_secs_f64() * 1000. / 20.;
        // Same-size empty scene: fixed cost of GPU sync + 19 MB readback + conversion.
        let start = std::time::Instant::now();
        for _ in 0..20 {
            gpu.render(&mut painter, &[], [0., 0., 0., 1.]).unwrap();
        }
        let overhead = start.elapsed().as_secs_f64() * 1000. / 20.;
        eprintln!(
            "{screen:?}: readback overhead={overhead:.2}ms, estimated GPU draw={:.2}ms",
            ms - overhead
        );
        gpu.render(&mut painter, &batches, [0., 0., 0., 1.])
            .unwrap();
        let slots = gpu.last_spill_slots();
        eprintln!("{screen:?}: spill_slots={slots} gpu+readback={ms:.2}ms software={sw:.2}ms");
        assert!(
            slots <= 2048,
            "{screen:?} needs {slots} blend-spill slots at rest"
        );
        assert_eq!(
            gpu.last_over_budget(),
            None,
            "{screen:?} fell back to software"
        );
    }
}

/// Incremental software frames must match a full repaint pixel for pixel,
/// while repainting only a small region for local changes such as hover.
#[test]
fn incremental_software_frames_match_full_repaints() {
    use volt_ui_kit::native::Damage;
    let mut app = App::new();
    let viewport = Viewport::new(1320., 900., 1.).unwrap();
    let (document, mut handlers) = view(&app).build().unwrap().into_parts();
    let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
    let mut live = RasterPainter::new(viewport);
    let mut reference = RasterPainter::new(viewport);
    runtime
        .layout(viewport, &Bindings::default(), &mut live)
        .unwrap();
    runtime.settle_animations();
    let ui = theme(&app, ThemeMode::Dark);
    let bg = [0.06, 0.11, 0.15, 1.];
    let mut check = |runtime: &mut Runtime, live: &mut RasterPainter, label: &str| -> Damage {
        // Animation targets resolve during paint (as in the live window), so
        // paint once, settle, then paint the steady state.
        runtime.paint_ui(live, &ui);
        runtime.settle_animations();
        let batches = runtime.paint_ui(live, &ui);
        let (pixels, damage) = live.rasterize_incremental(&batches, bg).unwrap();
        let pixels = pixels.clone();
        let fresh = runtime.paint_ui(&mut reference, &ui);
        let full = reference.rasterize(&fresh, bg).unwrap();
        assert!(
            pixels.data() == full.data(),
            "{label}: incremental frame differs from full repaint"
        );
        damage
    };
    assert_eq!(check(&mut runtime, &mut live, "first"), Damage::Full);
    assert_eq!(check(&mut runtime, &mut live, "unchanged"), Damage::None);
    // Hover a place card: only that card (and its shadow) repaints.
    let b = runtime
        .boxes()
        .iter()
        .find(|b| b.id == "place-1")
        .unwrap()
        .bounds
        .bounds();
    runtime.pointer_move(b[0] + 20., b[1] + 20.);
    match check(&mut runtime, &mut live, "hover") {
        Damage::Rect([x0, y0, x1, y1]) => {
            assert!(
                u64::from(x1 - x0) * u64::from(y1 - y0) < 1320 * 900 / 4,
                "hover repainted too much"
            );
        }
        other => panic!("hover should be a partial repaint, got {other:?}"),
    }
    runtime.pointer_move(5., 890.);
    check(&mut runtime, &mut live, "hover out");
    // Pressing and clicking changes app state and recomposes the tree.
    runtime.pointer_move(b[0] + 20., b[1] + 20.);
    runtime.pointer_down(b[0] + 20., b[1] + 20.);
    check(&mut runtime, &mut live, "pressed");
    assert!(
        runtime
            .pointer_up_outcome(b[0] + 20., b[1] + 20., &mut handlers, &mut app)
            .unwrap()
            .activated
    );
    let (document, next) = view(&app).build().unwrap().into_parts();
    runtime
        .replace_document(document, &next, &Bindings::default())
        .unwrap();
    handlers = next;
    runtime
        .layout(viewport, &Bindings::default(), &mut live)
        .unwrap();
    check(&mut runtime, &mut live, "selected another place");
    // Scrolling moves everything inside the page.
    runtime.scroll_at(600., 500., [0., 120.]);
    runtime
        .layout(viewport, &Bindings::default(), &mut live)
        .unwrap();
    check(&mut runtime, &mut live, "scrolled");
    let _ = &handlers;
}

#[test]
#[ignore = "timing report"]
fn incremental_hover_cost_at_retina() {
    let app = App::new();
    let viewport = Viewport::new(2640., 1800., 2.).unwrap();
    let (document, handlers) = view(&app).build().unwrap().into_parts();
    let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
    let mut p = RasterPainter::new(viewport);
    runtime
        .layout(viewport, &Bindings::default(), &mut p)
        .unwrap();
    let ui = theme(&app, ThemeMode::Dark);
    let bg = [0.06, 0.11, 0.15, 1.];
    let b = runtime
        .boxes()
        .iter()
        .find(|b| b.id == "place-1")
        .unwrap()
        .bounds
        .bounds();
    let (inside, outside) = ([b[0] + 40., b[1] + 40.], [10., 1790.]);
    let frame = |at: [f32; 2], p: &mut RasterPainter, runtime: &mut Runtime| {
        runtime.pointer_move(at[0], at[1]);
        runtime.paint_ui(p, &ui);
        runtime.settle_animations();
        let batches = runtime.paint_ui(p, &ui);
        let start = std::time::Instant::now();
        let (_, damage) = p.rasterize_incremental(&batches, bg).unwrap();
        (start.elapsed().as_secs_f64() * 1000., damage)
    };
    frame(outside, &mut p, &mut runtime);
    let (mut inc, mut n) = (0., 0);
    for i in 0..20 {
        let (ms, _) = frame(
            if i % 2 == 0 { inside } else { outside },
            &mut p,
            &mut runtime,
        );
        inc += ms;
        n += 1;
    }
    let batches = runtime.paint_ui(&mut p, &ui);
    let start = std::time::Instant::now();
    for _ in 0..10 {
        p.rasterize(&batches, bg).unwrap();
    }
    let full = start.elapsed().as_secs_f64() * 100.;
    eprintln!(
        "hover frame: incremental {:.2}ms vs full {:.2}ms",
        inc / n as f64,
        full
    );
}

#[test]
fn custom_window_controls_fit_existing_header_and_reset_preserves_host_state() {
    use volt_ui_kit::native::{WindowChrome, WindowState};
    let mut app = App::new();
    app.chrome = WindowChrome::Custom;
    app.window_control_style = volt_ui_kit::ui::WindowControlStyle::TrafficLights;
    app.window_state.resizable = true;
    for macos in [false, true] {
        for size in [[560., 460.], [760., 900.], [1320., 900.]] {
            // Exercise the Linux/Windows header geometry on every test host.
            let viewport = Viewport::new(size[0] - 84., size[1], 1.).unwrap();
            let ui: volt_ui_kit::ui::Element<App> =
                volt_ui_kit::vstack![screens::top_bar(&app, macos), volt_ui_kit::ui::spacer()]
                    .into();
            let (document, handlers) = ui.build().unwrap().into_parts();
            let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
            runtime
                .layout(
                    viewport,
                    &Bindings::default(),
                    &mut RasterPainter::new(viewport),
                )
                .unwrap();
            let labels: Vec<_> = runtime
                .accessibility_nodes()
                .into_iter()
                .filter(|n| {
                    ["Minimize window", "Maximize window", "Close window"]
                        .contains(&n.label.as_str())
                })
                .collect();
            assert_eq!(labels.len(), if macos { 0 } else { 3 });
            for n in labels {
                let [x, y, w, h] = runtime
                    .boxes()
                    .iter()
                    .find(|b| b.id == n.id)
                    .unwrap()
                    .bounds
                    .bounds();
                assert!(
                    x >= 0. && x + w <= viewport.width && y >= 0. && y + h <= 92.,
                    "{} outside header at {size:?}: {x},{y},{w},{h}",
                    n.label
                );
                assert!(!runtime.drag_region_hit(x + w / 2., y + h / 2.));
            }
        }
    }
    app.window_state.maximized = true;
    let before = app.window_state;
    app.reset();
    assert_eq!(app.chrome, WindowChrome::Custom);
    assert_eq!(
        app.window_control_style,
        volt_ui_kit::ui::WindowControlStyle::TrafficLights
    );
    assert_eq!(app.window_state, before);
    app.window_state.fullscreen = true;
    let runtime = mount(&app, [1320., 900.]);
    assert!(!runtime
        .accessibility_nodes()
        .iter()
        .any(|n| n.label == "Close window"));
    app.window_state = WindowState::default();
    app.chrome = WindowChrome::Native;
    let runtime = mount(&app, [1320., 900.]);
    assert!(!runtime
        .accessibility_nodes()
        .iter()
        .any(|n| n.label == "Close window"));
}

#[test]
fn raised_controls_move_hit_targets_without_moving_other_header_content() {
    use volt_ui_kit::{native::WindowChrome, ui::prelude::*};
    let mut app = App::new();
    app.chrome = WindowChrome::Custom;
    app.window_state.resizable = true;
    for mode in [ThemeMode::Light, ThemeMode::Dark] {
        app.mode = mode;
        for screen in Screen::ALL {
            app.screen = screen;
            for maximized in [false, true] {
                app.window_state.maximized = maximized;
                for width in [560., 760., 1320.] {
                    for scale in [1., 1.25, 2.] {
                        let vp = Viewport::new((width - 84.) * scale, 460. * scale, scale).unwrap();
                        // Same original row, including its old trailing controls.
                        // Compare against it, not guessed title/search coordinates.
                        let old = screens::header_row(
                            &app,
                            Some(
                                window_controls(maximized, |_: &mut App, _| {})
                                    .key("old-window-controls")
                                    .into(),
                            ),
                        );
                        let mount_header = |header: Element<App>| {
                            let ui: Element<App> = vstack![header, spacer()].into();
                            let (document, handlers) = ui.build().unwrap().into_parts();
                            let mut r =
                                Runtime::new(document, &handlers, &Bindings::default()).unwrap();
                            r.layout(vp, &Bindings::default(), &mut RasterPainter::new(vp))
                                .unwrap();
                            (r, handlers)
                        };
                        let (before, _) = mount_header(old);
                        let (mut after, mut handlers) = mount_header(screens::top_bar(&app, false));
                        let prefix = |r: &Runtime, stop: &str| {
                            r.boxes()
                                .iter()
                                .skip_while(|b| b.id != "top-bar")
                                .take_while(|b| b.id != stop)
                                .map(|b| (b.kind, b.bounds, b.clip, b.text_style.clone()))
                                .collect::<Vec<_>>()
                        };
                        assert_eq!(
                            prefix(&before, "old-window-controls"),
                            prefix(&after, "window-controls-slot"),
                            "non-control layout moved at {width}px / {scale}x / {screen:?}"
                        );
                        if width == 1320. && scale == 1.25 && !maximized {
                            let mut painter = RasterPainter::new(vp);
                            let palette = theme(&app, mode);
                            let old_batches = before.paint_ui(&mut painter, &palette);
                            let old_pixels = painter
                                .rasterize(&old_batches, palette.palette.base[0])
                                .unwrap();
                            let new_batches = after.paint_ui(&mut painter, &palette);
                            let new_pixels = painter
                                .rasterize(&new_batches, palette.palette.base[0])
                                .unwrap();
                            let left = after
                                .boxes()
                                .iter()
                                .find(|b| b.id == "northbound-window-controls")
                                .unwrap()
                                .bounds
                                .bounds()[0];
                            for y in 0..(92. * scale) as u32 {
                                for x in 0..(left - 2. * scale) as u32 {
                                    assert_eq!(
                                        old_pixels.pixel(x, y),
                                        new_pixels.pixel(x, y),
                                        "non-control pixels changed at {x},{y} in {screen:?}"
                                    );
                                }
                            }
                        }
                        let labels = [
                            "Minimize window",
                            if maximized {
                                "Restore window"
                            } else {
                                "Maximize window"
                            },
                            "Close window",
                        ];
                        for label in labels {
                            let bounds = |r: &Runtime| {
                                let node = r
                                    .accessibility_nodes()
                                    .into_iter()
                                    .find(|n| n.label == label)
                                    .unwrap();
                                r.boxes()
                                    .iter()
                                    .find(|b| b.id == node.id)
                                    .unwrap()
                                    .bounds
                                    .bounds()
                            };
                            let [ox, oy, ow, oh] = bounds(&before);
                            let [x, y, w, h] = bounds(&after);
                            for (a, b) in [(x, ox), (w, ow), (h, oh)] {
                                assert!((a - b).abs() < 0.001);
                            }
                            assert!(
                                (y + h / 2. - 26. * scale).abs() < 0.001,
                                "control center {y}+{h}/2 at {scale}x"
                            );
                            assert!(
                                (oy - y - 29. * scale).abs() < 0.001,
                                "only controls should rise by 29 logical px"
                            );
                            assert!(!after.drag_region_hit(x + w / 2., y + h / 2.));
                            after
                                .pointer_down_outcome(
                                    x + w / 2.,
                                    y + h / 2.,
                                    &mut handlers,
                                    &mut app,
                                )
                                .unwrap();
                            assert!(
                                after
                                    .pointer_up_outcome(
                                        x + w / 2.,
                                        y + h / 2.,
                                        &mut handlers,
                                        &mut app
                                    )
                                    .unwrap()
                                    .activated
                            );
                            // The old on-screen location must not remain clickable.
                            after
                                .pointer_down_outcome(
                                    ox + ow / 2.,
                                    oy + oh / 2.,
                                    &mut handlers,
                                    &mut app,
                                )
                                .unwrap();
                            assert!(
                                !after
                                    .pointer_up_outcome(
                                        ox + ow / 2.,
                                        oy + oh / 2.,
                                        &mut handlers,
                                        &mut app
                                    )
                                    .unwrap()
                                    .activated
                            );
                            assert!(after.drag_region_hit(ox + ow / 2., oy + oh / 2.));
                        }
                        if let Some(b) = after.boxes().iter().find(|b| b.id == "search") {
                            let [x, y, _, _] = b.bounds.bounds();
                            // The raised lane overlaps this y: its empty area must
                            // not occlude the unchanged search editor underneath.
                            let point = [x + 8. * scale, y + 5. * scale];
                            after
                                .pointer_down_outcome(point[0], point[1], &mut handlers, &mut app)
                                .unwrap();
                            after
                                .pointer_up_outcome(point[0], point[1], &mut handlers, &mut app)
                                .unwrap();
                            assert!(after.editing_focus());
                        }
                        assert_eq!(
                            after
                                .boxes()
                                .iter()
                                .find(|b| b.id == "top-bar")
                                .unwrap()
                                .bounds
                                .bounds()[3],
                            92. * scale
                        );
                    }
                }
            }
        }
    }
}
