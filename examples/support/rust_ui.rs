use std::{path::PathBuf, time::Instant};
use volt_ui_kit::{
    declarative::{Bindings, Kind, Runtime},
    native::{run_ui, NativeOptions, RasterPainter},
    ui::Element,
    Theme, Viewport,
};

/// Rust examples share the native host. Source edits require a Rust rebuild.
pub fn launch<C: 'static>(
    title: &str,
    mut state: C,
    view: impl Fn(&C) -> Element<C> + 'static,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut snapshot = None;
    let mut scale = 1_f32;
    let mut bench = None;
    let mut target = None;
    let mut size = [960_u32, 760_u32];
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--snapshot" => {
                snapshot = Some(PathBuf::from(args.next().ok_or("missing --snapshot path")?))
            }
            "--scale" => scale = args.next().ok_or("missing --scale value")?.parse()?,
            "--size" => {
                let value = args.next().ok_or("missing --size WIDTHxHEIGHT")?;
                let (width, height) = value.split_once('x').ok_or("size must be WIDTHxHEIGHT")?;
                size = [width.parse()?, height.parse()?];
            }
            "--bench" => {
                bench = Some(
                    args.next()
                        .ok_or("missing --bench count")?
                        .parse::<usize>()?,
                )
            }
            "--bench-target" => target = Some(args.next().ok_or("missing --bench-target key")?),
            "--help" => {
                println!("{title}\n--snapshot PNG    render without opening a window\n--size WIDTHxHEIGHT logical window/snapshot/benchmark size (default 960x760)\n--scale NUMBER    snapshot/benchmark scale (0.25..=4)\n--bench COUNT     simulate actions including Rust composition; warm CPU timings\n--bench-target KEY choose an enabled button/checkbox/toggle\nVOLT_UI_TRACE=1   native frame/event-to-present diagnostics\nRust source edits require rebuild; no JSON or F5 source reload.");
                return Ok(());
            }
            _ => {
                return Err(
                    format!("unknown option {arg}; Rust examples do not accept --document").into(),
                )
            }
        }
    }
    if !scale.is_finite() || !(0.25..=4.).contains(&scale) {
        return Err("scale must be in [0.25, 4]".into());
    }
    if size.contains(&0) || size.iter().any(|&v| v as f32 * scale > 16384.) {
        return Err("size must be positive and scaled dimensions must be <=16384".into());
    }
    if snapshot.is_some() && bench.is_some() {
        return Err("--snapshot and --bench are mutually exclusive".into());
    }
    if target.is_some() && bench.is_none() {
        return Err("--bench-target requires --bench".into());
    }
    if let Some(count) = bench {
        if !(1..=1000).contains(&count) {
            return Err("benchmark count must be in [1, 1000]".into());
        }
    }
    if snapshot.is_none() && bench.is_none() {
        return run_ui(
            NativeOptions {
                title: title.into(),
                size,
                document_path: None,
            },
            state,
            view,
        );
    }
    let values = Bindings::default();
    let (document, mut handlers) = view(&state).build()?.into_parts();
    let mut runtime = Runtime::new(document, &handlers, &values)?;
    let viewport = Viewport::new(size[0] as f32 * scale, size[1] as f32 * scale, scale).unwrap();
    let mut painter = RasterPainter::new(viewport);
    let theme = Theme::dark();
    runtime.layout(viewport, &values, &mut painter)?;
    if let Some(path) = snapshot {
        let batches = runtime.paint(&mut painter, &theme);
        painter
            .rasterize(&batches, theme.background)?
            .save_png(&path)?;
        println!(
            "Rendered {} Rust nodes to {} (not native input proof)",
            runtime.boxes().len(),
            path.display()
        );
        return Ok(());
    }
    let count = bench.unwrap();
    let target = runtime
        .boxes()
        .iter()
        .find(|b| {
            b.enabled
                && match &target {
                    Some(id) => {
                        b.id == *id
                            && matches!(b.kind, Kind::Button | Kind::Checkbox | Kind::Toggle)
                    }
                    None => b.kind == Kind::Button,
                }
        })
        .ok_or("benchmark target must be an enabled interactive control")?
        .id
        .clone();
    let mut packed = vec![0_u32; (viewport.width * viewport.height) as usize];
    let mut samples: Vec<[f64; 5]> = Vec::with_capacity(count);
    for i in 0..count + 4 {
        let area = runtime
            .boxes()
            .iter()
            .find(|b| b.id == target)
            .ok_or("benchmark target removed")?
            .bounds;
        let [x, y, w, h] = area.bounds();
        let (x, y) = (x + w * 0.5, y + h * 0.5);
        runtime.pointer_move(x, y);
        runtime.pointer_down(x, y);
        if runtime.focused() != Some(target.as_str()) {
            return Err("benchmark target is clipped or not hittable".into());
        }
        let pressed = runtime.paint(&mut painter, &theme);
        painter.rasterize(&pressed, theme.background)?;
        let start = Instant::now();
        let outcome = runtime.pointer_up_outcome(x, y, &mut handlers, &mut state)?;
        if !outcome.activated {
            return Err("benchmark target did not activate".into());
        }
        let (next, next_handlers) = view(&state).build()?.into_parts();
        runtime.replace_document(next, &next_handlers, &values)?;
        handlers = next_handlers;
        runtime.layout(viewport, &values, &mut painter)?;
        runtime.pointer_move(x, y);
        let compose = start.elapsed().as_secs_f64() * 1000.;
        let stage = Instant::now();
        let batches = runtime.paint(&mut painter, &theme);
        let paint = stage.elapsed().as_secs_f64() * 1000.;
        let stage = Instant::now();
        let pixmap = painter.rasterize(&batches, theme.background)?;
        let raster = stage.elapsed().as_secs_f64() * 1000.;
        let stage = Instant::now();
        for (dst, pixel) in packed.iter_mut().zip(pixmap.pixels()) {
            *dst = (u32::from(pixel.red()) << 16)
                | (u32::from(pixel.green()) << 8)
                | u32::from(pixel.blue());
        }
        std::hint::black_box(&packed);
        let pack = stage.elapsed().as_secs_f64() * 1000.;
        if i >= 4 {
            samples.push([
                compose,
                paint,
                raster,
                pack,
                start.elapsed().as_secs_f64() * 1000.,
            ]);
        }
    }
    println!("BENCH {title} {}x{} scale={scale} target={target} samples={count} warmup=4 (CPU only; excludes OS event queue/presentation)",viewport.width,viewport.height);
    for (column, name) in [
        "dispatch+compose+layout",
        "paint",
        "raster",
        "pack",
        "release-to-pixels",
    ]
    .iter()
    .enumerate()
    {
        let mut values: Vec<_> = samples.iter().map(|s| s[column]).collect();
        values.sort_by(f64::total_cmp);
        println!(
            "{name}: p50={:.3}ms p95={:.3}ms max={:.3}ms",
            values[(values.len() - 1) / 2],
            values[(values.len() as f64 * 0.95).ceil() as usize - 1],
            values[values.len() - 1]
        );
    }
    Ok(())
}
