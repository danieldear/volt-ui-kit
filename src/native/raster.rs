use crate::{
    declarative::{DocumentPainter, PaintBatch, TextMetrics, TextStyle},
    Painter, Viewport,
};
use cosmic_text::{
    Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, Style, SwashCache, Weight, Wrap,
};
use std::collections::BTreeMap;
use tiny_skia::{FillRule, Mask, Paint, PathBuilder, Pixmap, Stroke, Transform};

#[derive(Clone, PartialEq)]
pub enum Shape {
    Rectangle([f32; 4], [f32; 4]),
    Triangle([[f32; 2]; 3], [f32; 4]),
    Rounded([f32; 4], f32, [f32; 4]),
    Line([f32; 2], [f32; 2], f32, [f32; 4]),
    Corners([f32; 4], [f32; 4], [f32; 4]),
    Outline([f32; 4], [f32; 4], f32, [f32; 4]),
    Shadow([f32; 4], [f32; 4], f32, f32, [f32; 2], [f32; 4]),
    LayerBegin(f32),
    LayerEnd,
}

const EFFECT_CACHE_BYTES: usize = 8 * 1024 * 1024;
const EFFECT_CACHE_ENTRIES: usize = 32;
const EFFECT_TINT_BYTES: usize = 32 * 1024 * 1024;
const MAX_EFFECT_PIXELS: u64 = 4_194_304;
const MAX_FRAME_EFFECT_PIXELS: u64 = 16_777_216;
const MAX_LAYER_BYTES: u64 = 64 * 1024 * 1024;
const MAX_LAYER_DEPTH: usize = 24;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct EffectKey([u32; 9]);
struct EffectMask {
    width: u32,
    height: u32,
    data: Vec<u8>,
    used: u64,
    tint: Option<([u8; 3], Pixmap)>,
}
#[derive(Clone, PartialEq)]
pub struct Glyph {
    text: String,
    bounds: [f32; 4],
    style: TextStyle,
    color: [f32; 4],
}
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct TextKey {
    text: String,
    family: String,
    weight: u16,
    italic: bool,
    size: u32,
    width: u32,
}
fn default_style(size: f32) -> TextStyle {
    TextStyle {
        size,
        ..TextStyle::default()
    }
}
/// Shaped text buffers and glyph rasters are reused between event-driven frames.
pub struct RasterPainter {
    viewport: Viewport,
    fonts: FontSystem,
    system_family: String,
    swash: SwashCache,
    text: BTreeMap<TextKey, (Buffer, u64)>,
    clock: u64,
    glyph_bytes: usize,
    pub text_cache_misses: u64,
    /// Native shape commands after stroke batching (a command may use multiple spans).
    pub shape_draw_calls: u64,
    /// Alpha-mask cache misses. Keys exclude effect color and integer position.
    pub effect_cache_misses: u64,
    /// RGB tint rebuilds; alpha-only animation frames reuse premultiplied pixels.
    pub effect_tint_misses: u64,
    /// Transparent offscreen buffers are reused on subsequent animation frames.
    pub layer_allocations: u64,
    effect_masks: BTreeMap<EffectKey, EffectMask>,
    effect_bytes: usize,
    effect_tint_bytes: usize,
    layer_pool: Vec<Pixmap>,
}
impl RasterPainter {
    pub fn new(viewport: Viewport) -> Self {
        let mut fonts = FontSystem::new();
        configure_generic_families(&mut fonts);
        let system_family = system_family(&fonts);
        Self {
            viewport,
            fonts,
            system_family,
            swash: SwashCache::new(),
            text: BTreeMap::new(),
            text_cache_misses: 0,
            shape_draw_calls: 0,
            effect_cache_misses: 0,
            effect_tint_misses: 0,
            layer_allocations: 0,
            effect_masks: BTreeMap::new(),
            effect_bytes: 0,
            effect_tint_bytes: 0,
            layer_pool: Vec::new(),
            clock: 0,
            glyph_bytes: 0,
        }
    }
    pub fn set_viewport(&mut self, viewport: Viewport) {
        if self.viewport.width as u32 != viewport.width as u32
            || self.viewport.height as u32 != viewport.height as u32
        {
            self.layer_pool.clear();
        }
        self.viewport = viewport;
    }
    fn key(&mut self, text: &str, style: &TextStyle, width: f32) -> TextKey {
        let size = if style.size.is_finite() {
            style.size.clamp(1., 384.)
        } else {
            14.
        };
        let width = if width.is_finite() {
            width.clamp(0.1, 1_000_000.)
        } else {
            1.
        };
        let family = match style.family.as_str() {
            "system" | "" => self.system_family.clone(),
            name => name.to_owned(),
        };
        let key = TextKey {
            text: text.into(),
            family,
            weight: style.weight.clamp(1, 1000),
            italic: style.italic,
            size: size.to_bits(),
            width: width.to_bits(),
        };
        self.clock += 1;
        if !self.text.contains_key(&key) {
            if self.text.len() >= 256 {
                if let Some(oldest) = self
                    .text
                    .iter()
                    .min_by_key(|(_, (_, used))| used)
                    .map(|(key, _)| key.clone())
                {
                    self.text.remove(&oldest);
                }
            }
            let mut buffer = Buffer::new(&mut self.fonts, Metrics::new(size, size * 1.2));
            buffer.set_size(&mut self.fonts, Some(width.max(0.1)), None);
            buffer.set_wrap(&mut self.fonts, Wrap::WordOrGlyph);
            let family = match key.family.as_str() {
                "sans" | "sans-serif" => Family::SansSerif,
                "serif" => Family::Serif,
                "mono" | "monospace" => Family::Monospace,
                "cursive" => Family::Cursive,
                "fantasy" => Family::Fantasy,
                name => Family::Name(name),
            };
            let attrs = Attrs::new()
                .family(family)
                .weight(Weight(key.weight))
                .style(if key.italic {
                    Style::Italic
                } else {
                    Style::Normal
                });
            buffer.set_text(&mut self.fonts, text, &attrs, Shaping::Advanced);
            buffer.shape_until_scroll(&mut self.fonts, false);
            self.text.insert(key.clone(), (buffer, self.clock));
            self.text_cache_misses += 1;
        }
        self.text.get_mut(&key).unwrap().1 = self.clock;
        key
    }
    /// Blur uses three separable box passes approximating a Gaussian with
    /// sigma=blur/2. Coverage masks are color independent and bounded by an LRU.
    #[allow(clippy::too_many_arguments)]
    fn draw_shadow(
        &mut self,
        pixmap: &mut Pixmap,
        clip_mask: &Mask,
        clip: [f32; 4],
        bounds: [f32; 4],
        radii: [f32; 4],
        blur: f32,
        spread: f32,
        offset: [f32; 2],
        color: [f32; 4],
        frame_pixels: &mut u64,
    ) -> Result<(), String> {
        if !bounds.into_iter().all(f32::is_finite)
            || bounds[2] <= 0.
            || bounds[3] <= 0.
            || radii.into_iter().any(|r| !r.is_finite() || r < 0.)
            || ![blur, spread, offset[0], offset[1]]
                .into_iter()
                .all(f32::is_finite)
            || !(0. ..=384.).contains(&blur)
            || spread.abs() > 384.
            || offset.into_iter().any(|v| v.abs() > 65_536.)
            || color.into_iter().any(|v| !v.is_finite())
        {
            return Err("invalid or over-limit shadow geometry".into());
        }
        if color[3] <= 0. {
            return Ok(());
        }
        let ew = bounds[2] + spread * 2.;
        let eh = bounds[3] + spread * 2.;
        if ew <= 0. || eh <= 0. {
            return Ok(());
        }
        let radii = radii.map(|r| {
            (r.min(bounds[2].min(bounds[3]) * 0.5) + spread)
                .max(0.)
                .min(ew.min(eh) * 0.5)
        });
        let passes = gaussian_box_radii(blur * 0.5);
        let pad = passes.iter().sum::<u32>() + 2;
        let x = bounds[0] + offset[0] - spread;
        let y = bounds[1] + offset[1] - spread;
        let left = x.floor() - pad as f32;
        let top = y.floor() - pad as f32;
        let mask_width = (ew + (x - x.floor()) + 1.).ceil() + (pad * 2) as f32;
        let mask_height = (eh + (y - y.floor()) + 1.).ceil() + (pad * 2) as f32;
        if !mask_width.is_finite()
            || !mask_height.is_finite()
            || mask_width > MAX_EFFECT_PIXELS as f32
            || mask_height > MAX_EFFECT_PIXELS as f32
        {
            return Err("shadow mask dimensions exceed budget".into());
        }
        let mw = mask_width as u32;
        let mh = mask_height as u32;
        // A fully clipped effect performs no allocations or blur work.
        if left >= clip[0] + clip[2]
            || top >= clip[1] + clip[3]
            || left + mw as f32 <= clip[0]
            || top + mh as f32 <= clip[1]
        {
            return Ok(());
        }
        let area = u64::from(mw) * u64::from(mh);
        if area > MAX_EFFECT_PIXELS || frame_pixels.saturating_add(area) > MAX_FRAME_EFFECT_PIXELS {
            return Err("shadow mask/frame work budget exceeded".into());
        }
        *frame_pixels += area;
        let local = [x - left, y - top, ew, eh];
        let key = EffectKey([
            local[0].to_bits(),
            local[1].to_bits(),
            ew.to_bits(),
            eh.to_bits(),
            radii[0].to_bits(),
            radii[1].to_bits(),
            radii[2].to_bits(),
            radii[3].to_bits(),
            blur.to_bits(),
        ]);
        self.clock += 1;
        if !self.effect_masks.contains_key(&key) {
            let mut mask = Mask::new(mw, mh).ok_or("invalid effect mask allocation")?;
            let path = rounded_corners(local, radii).ok_or("invalid effect corner path")?;
            mask.fill_path(&path, FillRule::Winding, true, Transform::identity());
            let mut data = mask.data().to_vec();
            drop(mask);
            let mut scratch = vec![0_u8; area as usize];
            for radius in passes {
                if radius > 0 {
                    box_blur(
                        &data,
                        &mut scratch,
                        mw as usize,
                        mh as usize,
                        radius as usize,
                        true,
                    );
                    box_blur(
                        &scratch,
                        &mut data,
                        mw as usize,
                        mh as usize,
                        radius as usize,
                        false,
                    );
                }
            }
            while self.effect_masks.len() >= EFFECT_CACHE_ENTRIES
                || self.effect_bytes + data.len() > EFFECT_CACHE_BYTES
            {
                let Some(oldest) = self
                    .effect_masks
                    .iter()
                    .min_by_key(|(_, mask)| mask.used)
                    .map(|(key, _)| key.clone())
                else {
                    break;
                };
                let removed = self.effect_masks.remove(&oldest).unwrap();
                self.effect_bytes -= removed.data.len();
                if let Some((_, tint)) = removed.tint {
                    self.effect_tint_bytes -= tint.data().len();
                }
            }
            self.effect_bytes += data.len();
            self.effect_masks.insert(
                key.clone(),
                EffectMask {
                    width: mw,
                    height: mh,
                    data,
                    used: self.clock,
                    tint: None,
                },
            );
            self.effect_cache_misses += 1;
        }
        let source = [byte(color[0]), byte(color[1]), byte(color[2])];
        if self.effect_masks[&key]
            .tint
            .as_ref()
            .is_none_or(|(rgb, _)| *rgb != source)
        {
            if let Some((_, previous)) = self.effect_masks.get_mut(&key).unwrap().tint.take() {
                self.effect_tint_bytes -= previous.data().len();
            }
            let tint_bytes = area as usize * 4;
            while self.effect_tint_bytes + tint_bytes > EFFECT_TINT_BYTES {
                let Some(oldest) = self
                    .effect_masks
                    .iter()
                    .filter(|(_, m)| m.tint.is_some())
                    .min_by_key(|(_, m)| m.used)
                    .map(|(key, _)| key.clone())
                else {
                    return Err("effect tint cache budget exceeded".into());
                };
                if let Some((_, evicted)) = self.effect_masks.get_mut(&oldest).unwrap().tint.take()
                {
                    self.effect_tint_bytes -= evicted.data().len();
                }
            }
            let effect = &self.effect_masks[&key];
            let mut tint =
                Pixmap::new(effect.width, effect.height).ok_or("invalid effect tint allocation")?;
            let table: [_; 256] = std::array::from_fn(|coverage| {
                tiny_skia::ColorU8::from_rgba(source[0], source[1], source[2], coverage as u8)
                    .premultiply()
            });
            for (pixel, coverage) in tint.pixels_mut().iter_mut().zip(&effect.data) {
                *pixel = table[*coverage as usize];
            }
            self.effect_tint_bytes += tint.data().len();
            self.effect_masks.get_mut(&key).unwrap().tint = Some((source, tint));
            self.effect_tint_misses += 1;
        }
        let effect = self.effect_masks.get_mut(&key).unwrap();
        effect.used = self.clock;
        blit_tint(
            pixmap,
            &effect.tint.as_ref().unwrap().1,
            clip_mask,
            [left as i32, top as i32],
            clip,
            byte(color[3]),
        );
        Ok(())
    }
    pub fn rasterize(
        &mut self,
        batches: &[PaintBatch<Shape, Glyph>],
        background: [f32; 4],
    ) -> Result<Pixmap, String> {
        let w = self.viewport.width as u32;
        let h = self.viewport.height as u32;
        if u64::from(w) * u64::from(h) > 16_777_216 {
            return Err("framebuffer exceeds 16 megapixel limit".into());
        }
        let mut pixmap = Pixmap::new(w, h).ok_or("invalid framebuffer")?;
        pixmap.fill(
            tiny_skia::Color::from_rgba(background[0], background[1], background[2], 1.)
                .ok_or("invalid background color")?,
        );
        let mut mask = Mask::new(w, h).ok_or("invalid clip mask")?;
        let mut last_clip = None;
        let mut layers: Vec<(Pixmap, f32)> = Vec::new();
        let mut effect_pixels = 0_u64;
        for batch in batches {
            let [x, y, cw, ch] = batch.clip.bounds();
            if !batch.shapes.is_empty() && last_clip != Some([x, y, cw, ch]) {
                // One reusable rectangular mask; empty/text-only batches do not touch it.
                if let Some([lx, ly, lw, lh]) = last_clip {
                    fill_mask(&mut mask, w, h, [lx, ly, lw, lh], 0);
                }
                fill_mask(&mut mask, w, h, [x, y, cw, ch], 255);
                last_clip = Some([x, y, cw, ch]);
            }
            let mut shapes = batch.shapes.iter().peekable();
            while let Some(shape) = shapes.next() {
                match shape {
                    Shape::LayerBegin(opacity) => {
                        if !opacity.is_finite() || !(0. ..=1.).contains(opacity) {
                            return Err("invalid layer opacity".into());
                        }
                        if layers.len() >= MAX_LAYER_DEPTH
                            || u64::from(w) * u64::from(h) * 4 * (layers.len() as u64 + 2)
                                > MAX_LAYER_BYTES
                        {
                            return Err("opacity layer depth/memory budget exceeded".into());
                        }
                        let next = if let Some(mut reusable) = self.layer_pool.pop() {
                            reusable.data_mut().fill(0);
                            reusable
                        } else {
                            self.layer_allocations += 1;
                            Pixmap::new(w, h).ok_or("invalid opacity layer")?
                        };
                        layers.push((std::mem::replace(&mut pixmap, next), *opacity));
                        continue;
                    }
                    Shape::LayerEnd => {
                        let (mut parent, opacity) =
                            layers.pop().ok_or("unbalanced opacity layer end")?;
                        composite_layer(&mut parent, &pixmap, byte(opacity));
                        self.layer_pool.push(std::mem::replace(&mut pixmap, parent));
                        continue;
                    }
                    Shape::Shadow(bounds, radii, blur, spread, offset, color) => {
                        self.draw_shadow(
                            &mut pixmap,
                            &mask,
                            [x, y, cw, ch],
                            *bounds,
                            *radii,
                            *blur,
                            *spread,
                            *offset,
                            *color,
                            &mut effect_pixels,
                        )?;
                        self.shape_draw_calls += 1;
                        continue;
                    }
                    Shape::Outline(bounds, radii, width, color) => {
                        if !width.is_finite() || *width <= 0. {
                            continue;
                        }
                        let Some(path) = outline(*bounds, *radii, *width) else {
                            continue;
                        };
                        pixmap.fill_path(
                            &path,
                            &shape_paint(*color),
                            FillRule::EvenOdd,
                            Transform::identity(),
                            Some(&mask),
                        );
                        self.shape_draw_calls += 1;
                        continue;
                    }
                    _ => {}
                }
                // Legacy panels emit short line segments around their border. Batch
                // same-style consecutive segments into one native stroked path.
                if let Shape::Line(a, b, width, color) = shape {
                    let mut builder = PathBuilder::new();
                    builder.move_to(a[0], a[1]);
                    builder.line_to(b[0], b[1]);
                    let mut last_point = *b;
                    while let Some(Shape::Line(a, b, next_width, next_color)) = shapes.peek() {
                        if next_width != width || next_color != color {
                            break;
                        }
                        if *a != last_point {
                            builder.move_to(a[0], a[1]);
                        }
                        builder.line_to(b[0], b[1]);
                        last_point = *b;
                        shapes.next();
                    }
                    if let Some(path) = builder.finish() {
                        let paint = shape_paint(*color);
                        let stroke = Stroke {
                            width: *width,
                            ..Stroke::default()
                        };
                        pixmap.stroke_path(
                            &path,
                            &paint,
                            &stroke,
                            Transform::identity(),
                            Some(&mask),
                        );
                        self.shape_draw_calls += 1;
                    }
                    continue;
                }
                let (path, color) = match shape {
                    Shape::Rectangle([x, y, w, h], c) => {
                        let Some(r) = tiny_skia::Rect::from_xywh(*x, *y, *w, *h) else {
                            continue;
                        };
                        (PathBuilder::from_rect(r), *c)
                    }
                    Shape::Rounded(bounds, radius, c) => {
                        if layers.is_empty()
                            && fill_rounded_fast(
                                &mut pixmap,
                                &mask,
                                [x, y, cw, ch],
                                *bounds,
                                *radius,
                                *c,
                            )
                        {
                            self.shape_draw_calls += 1;
                            continue;
                        }
                        let Some(path) = rounded(*bounds, *radius) else {
                            continue;
                        };
                        (path, *c)
                    }
                    Shape::Line(..) => unreachable!("lines handled above"),
                    Shape::Corners(bounds, radii, c) => {
                        let Some(path) = rounded_corners(*bounds, *radii) else {
                            continue;
                        };
                        (path, *c)
                    }
                    Shape::Outline(..)
                    | Shape::Shadow(..)
                    | Shape::LayerBegin(..)
                    | Shape::LayerEnd => unreachable!("effect commands handled above"),
                    Shape::Triangle(points, c) => {
                        let mut p = PathBuilder::new();
                        p.move_to(points[0][0], points[0][1]);
                        p.line_to(points[1][0], points[1][1]);
                        p.line_to(points[2][0], points[2][1]);
                        p.close();
                        let Some(path) = p.finish() else { continue };
                        (path, *c)
                    }
                };
                let paint = shape_paint(color);
                pixmap.fill_path(
                    &path,
                    &paint,
                    FillRule::Winding,
                    Transform::identity(),
                    Some(&mask),
                );
                self.shape_draw_calls += 1;
            }
            for glyph in &batch.glyphs {
                let key = self.key(&glyph.text, &glyph.style, glyph.bounds[2]);
                let buffer = &self.text[&key].0;
                let [gx, gy, gw, gh] = glyph.bounds;
                let [cx, cy, cw, ch] = batch.clip.bounds();
                let c = glyph.color;
                for run in buffer.layout_runs() {
                    if run.line_top >= gh || run.line_top + run.line_height <= 0. {
                        continue;
                    }
                    for shaped in run.glyphs {
                        if shaped.x >= gw || shaped.x + shaped.w < 0. {
                            continue;
                        }
                        let physical = shaped.physical((0., 0.), 1.);
                        if self.glyph_bytes > 16 * 1024 * 1024
                            || self.swash.image_cache.len() > 4096
                        {
                            self.swash.image_cache.clear();
                            self.swash.outline_command_cache.clear();
                            self.glyph_bytes = 0;
                        }
                        if !self.swash.image_cache.contains_key(&physical.cache_key) {
                            self.glyph_bytes += self
                                .swash
                                .get_image(&mut self.fonts, physical.cache_key)
                                .as_ref()
                                .map_or(0, |image| image.data.len());
                        }
                        self.swash.with_pixels(
                            &mut self.fonts,
                            physical.cache_key,
                            Color::rgba(byte(c[0]), byte(c[1]), byte(c[2]), 255),
                            |x, y, color| {
                                let px = x + physical.x + gx.round() as i32;
                                let py = y + physical.y + run.line_y as i32 + gy.round() as i32;
                                if px < 0 || py < 0 || px >= w as i32 || py >= h as i32 {
                                    return;
                                }
                                let fx = px as f32;
                                let fy = py as f32;
                                if fx < cx
                                    || fy < cy
                                    || fx >= cx + cw
                                    || fy >= cy + ch
                                    || fx < gx
                                    || fy < gy
                                    || fx >= gx + gw
                                    || fy >= gy + gh
                                {
                                    return;
                                }
                                let [r, g, b, a] = color.as_rgba();
                                let alpha = a as f32 / 255. * c[3];
                                let pixel = &mut pixmap.pixels_mut()
                                    [py as usize * w as usize + px as usize];
                                blend_pixel(pixel, [r, g, b], alpha);
                            },
                        );
                    }
                }
            }
        }
        if !layers.is_empty() {
            return Err("unbalanced opacity layer begin".into());
        }
        Ok(pixmap)
    }
}
// cosmic-text's default generic families may not be installed (e.g. DejaVu
// Serif on macOS). Select available faces instead of silently using sans fallback.
fn configure_generic_families(fonts: &mut FontSystem) {
    fn installed(fonts: &FontSystem, candidates: &[&str]) -> Option<String> {
        candidates
            .iter()
            .find(|candidate| {
                fonts
                    .db()
                    .faces()
                    .any(|face| face.families.iter().any(|(name, _)| name == *candidate))
            })
            .map(|name| (*name).to_owned())
    }
    for (family, candidates) in [
        (
            "serif",
            &["Georgia", "Times New Roman", "DejaVu Serif", "Noto Serif"][..],
        ),
        (
            "sans",
            &[
                "Avenir Next",
                "DejaVu Sans",
                "Arial",
                "Noto Sans",
                "Open Sans",
            ][..],
        ),
        (
            "mono",
            &[
                "Menlo",
                "Consolas",
                "DejaVu Sans Mono",
                "Noto Sans Mono",
                "Courier New",
            ][..],
        ),
    ] {
        if let Some(name) = installed(fonts, candidates) {
            match family {
                "serif" => fonts.db_mut().set_serif_family(name),
                "sans" => fonts.db_mut().set_sans_serif_family(name),
                _ => fonts.db_mut().set_monospace_family(name),
            }
        }
    }
}
fn system_family(fonts: &FontSystem) -> String {
    if cfg!(target_os = "macos")
        && fonts
            .db()
            .faces()
            .any(|face| face.families.iter().any(|(name, _)| name == "Avenir Next"))
    {
        "Avenir Next".into()
    } else {
        "sans-serif".into()
    }
}
fn shape_paint(color: [f32; 4]) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color_rgba8(
        byte(color[0]),
        byte(color[1]),
        byte(color[2]),
        byte(color[3]),
    );
    paint
}
fn byte(v: f32) -> u8 {
    (v.clamp(0., 1.) * 255.).round() as u8
}
impl TextMetrics for RasterPainter {
    fn measure(&mut self, text: &str, size: f32, max_width: f32) -> [f32; 2] {
        self.measure_styled(text, &default_style(size), max_width)
    }
    fn measure_styled(&mut self, text: &str, style: &TextStyle, max_width: f32) -> [f32; 2] {
        let key = self.key(text, style, max_width);
        let buffer = &self.text[&key].0;
        let mut width = 0_f32;
        let mut height = f32::from_bits(key.size) * 1.2;
        for run in buffer.layout_runs() {
            width = width.max(run.line_w);
            height = height.max(run.line_top + run.line_height);
        }
        [width, height]
    }
}
impl Painter for RasterPainter {
    type Shape = Shape;
    type Glyph = Glyph;
    fn viewport(&self) -> Viewport {
        self.viewport
    }
    fn draw_rect(&self, out: &mut Vec<Shape>, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        out.push(Shape::Rectangle([x, y, w, h], color));
    }
    fn card_triangle(&self, out: &mut Vec<Shape>, points: [[f32; 2]; 3], color: [f32; 4]) {
        out.push(Shape::Triangle(points, color));
    }
    fn card_round(&self, out: &mut Vec<Shape>, rect: [f32; 4], radius: f32, color: [f32; 4]) {
        out.push(Shape::Rounded(rect, radius, color));
    }
    fn card_corners(&self, out: &mut Vec<Shape>, rect: [f32; 4], radii: [f32; 4], color: [f32; 4]) {
        out.push(Shape::Corners(rect, radii, color));
    }
    fn card_outline(
        &self,
        out: &mut Vec<Shape>,
        rect: [f32; 4],
        radii: [f32; 4],
        width: f32,
        color: [f32; 4],
    ) {
        out.push(Shape::Outline(rect, radii, width, color));
    }
    fn card_shadow(
        &self,
        out: &mut Vec<Shape>,
        rect: [f32; 4],
        radii: [f32; 4],
        blur: f32,
        spread: f32,
        offset: [f32; 2],
        color: [f32; 4],
    ) {
        out.push(Shape::Shadow(rect, radii, blur, spread, offset, color));
    }
    fn card_line(
        &self,
        out: &mut Vec<Shape>,
        a: [f32; 2],
        b: [f32; 2],
        width: f32,
        color: [f32; 4],
    ) {
        if width.is_finite() && width > 0. && a != b {
            out.push(Shape::Line(a, b, width, color));
        }
    }
    fn card_text(
        &mut self,
        out: &mut Vec<Glyph>,
        text: &str,
        x: f32,
        y: f32,
        size: f32,
        color: [f32; 4],
    ) {
        if !text.is_empty() {
            out.push(Glyph {
                text: text.into(),
                bounds: [x, y, self.viewport.width - x, size * 1.2],
                style: default_style(size),
                color,
            });
        }
    }
    fn draw_text_colored(
        &mut self,
        out: &mut Vec<Glyph>,
        text: &str,
        x: f32,
        y: f32,
        size: f32,
        _line_height: f32,
        color_at: &dyn Fn(usize) -> [f32; 4],
    ) {
        let mut cursor = x;
        for (i, ch) in text.char_indices() {
            let text = ch.to_string();
            let [w, _] = self.measure(&text, size, 1_000_000.);
            self.card_text(out, &text, cursor, y, size, color_at(i));
            cursor += w;
        }
    }
}
impl DocumentPainter for RasterPainter {
    fn ui_layer_begin(&mut self, out: &mut Vec<Shape>, opacity: f32) {
        out.push(Shape::LayerBegin(opacity));
    }
    fn ui_layer_end(&mut self, out: &mut Vec<Shape>) {
        out.push(Shape::LayerEnd);
    }
    fn ui_text(
        &mut self,
        out: &mut Vec<Glyph>,
        text: &str,
        bounds: [f32; 4],
        size: f32,
        color: [f32; 4],
    ) {
        self.ui_text_styled(out, text, bounds, &default_style(size), color);
    }
    fn ui_text_styled(
        &mut self,
        out: &mut Vec<Glyph>,
        text: &str,
        bounds: [f32; 4],
        style: &TextStyle,
        color: [f32; 4],
    ) {
        if bounds[2] > 0. && bounds[3] > 0. && !text.is_empty() {
            out.push(Glyph {
                text: text.into(),
                bounds,
                style: style.clone(),
                color,
            });
        }
    }
}

fn fill_mask(mask: &mut Mask, w: u32, h: u32, r: [f32; 4], value: u8) {
    let left = r[0].ceil().clamp(0., w as f32) as usize;
    let top = r[1].ceil().clamp(0., h as f32) as usize;
    let right = (r[0] + r[2]).ceil().clamp(left as f32, w as f32) as usize;
    let bottom = (r[1] + r[3]).ceil().clamp(top as f32, h as f32) as usize;
    for row in top..bottom {
        mask.data_mut()[row * w as usize + left..row * w as usize + right].fill(value);
    }
}
/// Split on integer scanlines: AA caps/edges retain tiny-skia's original cubic
/// geometry; fully covered middle pixels use a direct integer source-over span.
/// Disjoint regions guarantee translucent fills never overlap at split seams.
fn fill_rounded_fast(
    pixmap: &mut Pixmap,
    mask: &Mask,
    clip: [f32; 4],
    bounds: [f32; 4],
    radius: f32,
    color: [f32; 4],
) -> bool {
    let [x, y, w, h] = bounds;
    if !bounds.into_iter().all(f32::is_finite) || !radius.is_finite() || w <= 0. || h <= 0. {
        return false;
    }
    let r = radius.max(0.).min(w * 0.5).min(h * 0.5);
    let top = (y + r).ceil();
    let bottom = (y + h - r).floor();
    let left = x.ceil();
    let right = (x + w).floor();
    if top >= bottom || left >= right {
        return false;
    }
    let paint = shape_paint(color);
    for (is_top, cut) in [(true, top), (false, bottom)] {
        if let Some(path) = rounded_cap(bounds, r, cut, is_top) {
            pixmap.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                Some(mask),
            );
        }
    }
    // Preserve fractional side-edge AA exactly using narrow rectangular paths.
    for (sx, sw) in [(x, left - x), (right, x + w - right)] {
        if let Some(rect) = tiny_skia::Rect::from_xywh(sx, top, sw, bottom - top) {
            pixmap.fill_path(
                &PathBuilder::from_rect(rect),
                &paint,
                FillRule::Winding,
                Transform::identity(),
                Some(mask),
            );
        }
    }
    let fw = pixmap.width();
    let fh = pixmap.height();
    let l = left.max(clip[0].ceil()).clamp(0., fw as f32) as usize;
    let t = top.max(clip[1].ceil()).clamp(0., fh as f32) as usize;
    let rr = right
        .min((clip[0] + clip[2]).ceil())
        .clamp(l as f32, fw as f32) as usize;
    let bb = bottom
        .min((clip[1] + clip[3]).ceil())
        .clamp(t as f32, fh as f32) as usize;
    let source = tiny_skia::ColorU8::from_rgba(
        byte(color[0]),
        byte(color[1]),
        byte(color[2]),
        byte(color[3]),
    )
    .premultiply();
    let pixels = pixmap.pixels_mut();
    for row in t..bb {
        let span = &mut pixels[row * fw as usize + l..row * fw as usize + rr];
        if source.alpha() == 255 {
            span.fill(source);
        } else if source.alpha() > 0 {
            let inverse = 255_u32 - u32::from(source.alpha());
            for pixel in span {
                let blend = |s: u8, d: u8| -> u8 {
                    // Match tiny-skia 0.11's low-precision source-over:
                    // premultiplied source plus ceil(dst * inverse / 256).
                    let product = u32::from(d) * inverse;
                    (u32::from(s) + ((product + 255) >> 8)).min(255) as u8
                };
                *pixel = tiny_skia::PremultipliedColorU8::from_rgba(
                    blend(source.red(), pixel.red()),
                    blend(source.green(), pixel.green()),
                    blend(source.blue(), pixel.blue()),
                    255, // rasterize always begins with an opaque framebuffer
                )
                .expect("source-over preserves premultiplied channels");
            }
        }
    }
    true
}
fn rounded_cap([x, y, w, h]: [f32; 4], r: f32, cut: f32, top: bool) -> Option<tiny_skia::Path> {
    let k = r * 0.5522848;
    let mut p = PathBuilder::new();
    if top {
        p.move_to(x, cut);
        p.line_to(x, y + r);
        p.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
        p.line_to(x + w - r, y);
        p.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
        p.line_to(x + w, cut);
    } else {
        p.move_to(x, cut);
        p.line_to(x + w, cut);
        p.line_to(x + w, y + h - r);
        p.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
        p.line_to(x + r, y + h);
        p.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    }
    p.close();
    p.finish()
}

fn rounded([x, y, w, h]: [f32; 4], r: f32) -> Option<tiny_skia::Path> {
    if w <= 0. || h <= 0. {
        return None;
    }
    let r = r.max(0.).min(w * 0.5).min(h * 0.5);
    let k = r * 0.5522848;
    let mut p = PathBuilder::new();
    p.move_to(x + r, y);
    p.line_to(x + w - r, y);
    p.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    p.line_to(x + w, y + h - r);
    p.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    p.line_to(x + r, y + h);
    p.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    p.line_to(x, y + r);
    p.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    p.close();
    p.finish()
}

fn rounded_corners([x, y, w, h]: [f32; 4], radii: [f32; 4]) -> Option<tiny_skia::Path> {
    if ![x, y, w, h, x + w, y + h].into_iter().all(f32::is_finite)
        || w <= 0.
        || h <= 0.
        || radii.into_iter().any(|r| !r.is_finite() || r < 0.)
    {
        return None;
    }
    let [tl, tr, br, bl] = radii.map(|r| r.min(w.min(h) * 0.5));
    let k = 0.5522848;
    let mut p = PathBuilder::new();
    p.move_to(x + tl, y);
    p.line_to(x + w - tr, y);
    p.cubic_to(
        x + w - tr + tr * k,
        y,
        x + w,
        y + tr - tr * k,
        x + w,
        y + tr,
    );
    p.line_to(x + w, y + h - br);
    p.cubic_to(
        x + w,
        y + h - br + br * k,
        x + w - br + br * k,
        y + h,
        x + w - br,
        y + h,
    );
    p.line_to(x + bl, y + h);
    p.cubic_to(
        x + bl - bl * k,
        y + h,
        x,
        y + h - bl + bl * k,
        x,
        y + h - bl,
    );
    p.line_to(x, y + tl);
    p.cubic_to(x, y + tl - tl * k, x + tl - tl * k, y, x + tl, y);
    p.close();
    p.finish()
}

fn outline(bounds: [f32; 4], radii: [f32; 4], width: f32) -> Option<tiny_skia::Path> {
    let outer = rounded_corners(bounds, radii)?;
    if width * 2. >= bounds[2].min(bounds[3]) {
        return Some(outer);
    }
    let inner_bounds = [
        bounds[0] + width,
        bounds[1] + width,
        bounds[2] - width * 2.,
        bounds[3] - width * 2.,
    ];
    let limit = bounds[2].min(bounds[3]) * 0.5;
    let inner = rounded_corners(inner_bounds, radii.map(|r| (r.min(limit) - width).max(0.)))?;
    let mut builder = PathBuilder::new();
    builder.push_path(&outer);
    builder.push_path(&inner);
    builder.finish()
}

fn blend_pixel(pixel: &mut tiny_skia::PremultipliedColorU8, rgb: [u8; 3], alpha: f32) {
    let alpha = alpha.clamp(0., 1.);
    let inverse = 1. - alpha;
    let a = (alpha * 255. + pixel.alpha() as f32 * inverse).round() as u8;
    let blend = |source: u8, old: u8| {
        (source as f32 * alpha + old as f32 * inverse)
            .round()
            .min(a as f32) as u8
    };
    *pixel = tiny_skia::PremultipliedColorU8::from_rgba(
        blend(rgb[0], pixel.red()),
        blend(rgb[1], pixel.green()),
        blend(rgb[2], pixel.blue()),
        a,
    )
    .expect("source-over preserves premultiplied channels");
}

/// Integer clipped source-over spans. Coverage/RGB pixels stay cached; changing
/// effect opacity never walks the mask to rebuild tints or performs float math
/// per pixel. This avoids tiny-skia's masked pattern pipeline overhead here.
fn blit_tint(
    dest: &mut Pixmap,
    source: &Pixmap,
    mask: &Mask,
    position: [i32; 2],
    clip: [f32; 4],
    opacity: u8,
) {
    if opacity == 0 {
        return;
    }
    let [x, y] = position;
    let fw = dest.width() as usize;
    let left = x.max(clip[0].ceil() as i32).max(0).min(dest.width() as i32);
    let top = y
        .max(clip[1].ceil() as i32)
        .max(0)
        .min(dest.height() as i32);
    let right = x
        .saturating_add(source.width() as i32)
        .min((clip[0] + clip[2]).ceil() as i32)
        .min(dest.width() as i32)
        .max(left);
    let bottom = y
        .saturating_add(source.height() as i32)
        .min((clip[1] + clip[3]).ceil() as i32)
        .min(dest.height() as i32)
        .max(top);
    if left >= right || top >= bottom {
        return;
    }
    let opacity = u32::from(opacity);
    let scale = |value: u8| ((u32::from(value) * opacity + 127) / 255) as u8;
    let width = (right - left) as usize;
    for row in top..bottom {
        let di = row as usize * fw + left as usize;
        let si = (row - y) as usize * source.width() as usize + (left - x) as usize;
        let target = &mut dest.data_mut()[di * 4..(di + width) * 4];
        let input = &source.data()[si * 4..(si + width) * 4];
        let coverage = &mask.data()[di..di + width];
        for ((target, input), clip_alpha) in target
            .chunks_exact_mut(4)
            .zip(input.chunks_exact(4))
            .zip(coverage)
        {
            if *clip_alpha == 0 || input[3] == 0 {
                continue;
            }
            blend_premultiplied(target, input, &scale);
        }
    }
}

/// Group buffers share the framebuffer size. Skip transparent pixels instead
/// of running a pattern shader over the entire otherwise-empty framebuffer.
fn composite_layer(dest: &mut Pixmap, source: &Pixmap, opacity: u8) {
    if opacity == 0 {
        return;
    }
    debug_assert_eq!(dest.data().len(), source.data().len());
    let opacity = u32::from(opacity);
    let scale = |value: u8| ((u32::from(value) * opacity + 127) / 255) as u8;
    for (target, input) in dest
        .data_mut()
        .chunks_exact_mut(4)
        .zip(source.data().chunks_exact(4))
    {
        if input[3] != 0 {
            blend_premultiplied(target, input, &scale);
        }
    }
}

#[inline]
fn blend_premultiplied(target: &mut [u8], input: &[u8], scale: &impl Fn(u8) -> u8) {
    let a = scale(input[3]);
    let inverse = 255 - u32::from(a);
    for channel in 0..4 {
        let source = if channel == 3 {
            a
        } else {
            scale(input[channel])
        };
        let product = u32::from(target[channel]) * inverse;
        target[channel] = (u32::from(source) + ((product + 255) >> 8)).min(255) as u8;
    }
}

fn gaussian_box_radii(sigma: f32) -> [u32; 3] {
    if sigma <= 0. {
        return [0; 3];
    }
    let mut lower = (4. * sigma * sigma + 1.).sqrt().floor() as u32;
    if lower.is_multiple_of(2) {
        lower = lower.saturating_sub(1);
    }
    lower = lower.max(1);
    let lf = lower as f32;
    let count = ((12. * sigma * sigma - 3. * lf * lf - 12. * lf - 9.) / (-4. * lf - 4.))
        .round()
        .clamp(0., 3.) as usize;
    std::array::from_fn(|i| {
        if i < count {
            (lower - 1) / 2
        } else {
            lower.div_ceil(2)
        }
    })
}

fn box_blur(source: &[u8], out: &mut [u8], w: usize, h: usize, radius: usize, horizontal: bool) {
    let (lines, length) = if horizontal { (h, w) } else { (w, h) };
    let kernel = (radius * 2 + 1) as u32;
    for line in 0..lines {
        let index = |position: usize| {
            if horizontal {
                line * w + position
            } else {
                position * w + line
            }
        };
        let mut sum = (0..=radius.min(length - 1))
            .map(|i| u32::from(source[index(i)]))
            .sum::<u32>();
        for position in 0..length {
            out[index(position)] = ((sum + kernel / 2) / kernel) as u8;
            if position >= radius {
                sum -= u32::from(source[index(position - radius)]);
            }
            if position + radius + 1 < length {
                sum += u32::from(source[index(position + radius + 1)]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rect;
    #[test]
    fn cached_shaping_and_nonuniform_text_metrics() {
        let mut p = RasterPainter::new(Viewport::new(400., 300., 1.).unwrap());
        let wide = p.measure("WWW", 20., 400.);
        let narrow = p.measure("iii", 20., 400.);
        assert!(wide[0] > narrow[0]);
        let misses = p.text_cache_misses;
        p.measure("WWW", 20., 400.);
        assert_eq!(p.text_cache_misses, misses);
        for i in 0..300 {
            p.measure(&format!("entry{i}"), 12., 400.);
        }
        assert_eq!(p.text.len(), 256);
    }
    #[test]
    fn clipping_and_rounded_rasterization() {
        let mut p = RasterPainter::new(Viewport::new(80., 80., 1.).unwrap());
        let mut shapes = vec![];
        p.card_round(&mut shapes, [10., 10., 50., 50.], 10., [1., 0., 0., 1.]);
        let frame = p
            .rasterize(
                &[PaintBatch {
                    clip: Rect::new(0., 0., 35., 80.).unwrap(),
                    shapes,
                    glyphs: vec![],
                }],
                [0., 0., 0., 1.],
            )
            .unwrap();
        let pixel = |x: usize, y: usize| frame.pixels()[y * 80 + x];
        assert_eq!(pixel(10, 10).red(), 0);
        assert_eq!(pixel(25, 25).red(), 255);
        assert_eq!(pixel(40, 25).red(), 0);
    }
    #[test]
    fn selected_corners_fill_outline_and_blurred_shadow() {
        let mut p = RasterPainter::new(Viewport::new(80., 80., 1.).unwrap());
        let mut shapes = vec![];
        p.card_shadow(
            &mut shapes,
            [20., 20., 40., 40.],
            [12., 0., 0., 12.],
            4.,
            0.,
            [0., 0.],
            [1., 0., 0., 1.],
        );
        p.card_corners(
            &mut shapes,
            [20., 20., 40., 40.],
            [12., 0., 0., 12.],
            [0., 1., 0., 1.],
        );
        p.card_outline(
            &mut shapes,
            [20., 20., 40., 40.],
            [12., 0., 0., 12.],
            2.,
            [0., 0., 1., 1.],
        );
        let frame = p
            .rasterize(
                &[PaintBatch {
                    clip: Rect::new(0., 0., 80., 80.).unwrap(),
                    shapes: shapes.clone(),
                    glyphs: vec![],
                }],
                [0., 0., 0., 1.],
            )
            .unwrap();
        assert!(
            frame.pixels()[40 * 80 + 18].red() > 0,
            "blur extends beyond silhouette"
        );
        assert_eq!(frame.pixels()[40 * 80 + 40].green(), 255);
        assert!(frame.pixels()[20 * 80 + 58].blue() > 0);
        let misses = p.effect_cache_misses;
        p.rasterize(
            &[PaintBatch {
                clip: Rect::new(0., 0., 80., 80.).unwrap(),
                shapes,
                glyphs: vec![],
            }],
            [0., 0., 0., 1.],
        )
        .unwrap();
        assert_eq!(p.effect_cache_misses, misses);
    }
    #[test]
    fn nested_layer_opacity_is_grouped_and_budget_is_explicit() {
        let mut p = RasterPainter::new(Viewport::new(40., 40., 1.).unwrap());
        let batch = |shapes| PaintBatch {
            clip: Rect::new(0., 0., 40., 40.).unwrap(),
            shapes,
            glyphs: vec![],
        };
        let frame = p
            .rasterize(
                &[batch(vec![
                    Shape::LayerBegin(0.5),
                    Shape::Rectangle([0., 0., 30., 30.], [1., 0., 0., 1.]),
                    Shape::Rectangle([10., 0., 30., 30.], [0., 1., 0., 1.]),
                    Shape::LayerBegin(0.5),
                    Shape::Rectangle([10., 10., 10., 10.], [0., 0., 1., 1.]),
                    Shape::LayerEnd,
                    Shape::LayerEnd,
                ])],
                [0., 0., 0., 1.],
            )
            .unwrap();
        assert!(frame.pixels()[5 * 40 + 15].green().abs_diff(128) <= 1);
        assert!(frame.pixels()[15 * 40 + 15].blue().abs_diff(64) <= 1);
        assert!(p
            .rasterize(&[batch(vec![Shape::LayerEnd])], [0., 0., 0., 1.])
            .is_err());
        assert!(p
            .rasterize(&[batch(vec![Shape::LayerBegin(0.5); 25])], [0., 0., 0., 1.])
            .is_err());
    }
    #[test]
    fn shadow_cache_ignores_color_and_integer_translation_but_is_bounded() {
        let mut p = RasterPainter::new(Viewport::new(100., 100., 1.).unwrap());
        let make = |bounds, color| PaintBatch {
            clip: Rect::new(0., 0., 100., 100.).unwrap(),
            shapes: vec![Shape::Shadow(bounds, [4.; 4], 8., 1., [-2., 3.], color)],
            glyphs: vec![],
        };
        p.rasterize(
            &[make([20.25, 20.5, 30., 30.], [1., 0., 0., 0.5])],
            [0., 0., 0., 1.],
        )
        .unwrap();
        let misses = p.effect_cache_misses;
        p.rasterize(
            &[make([21.25, 22.5, 30., 30.], [0., 1., 0., 0.8])],
            [0., 0., 0., 1.],
        )
        .unwrap();
        assert_eq!(
            p.effect_cache_misses, misses,
            "color and integer offsets reuse coverage"
        );
        p.rasterize(
            &[make([21.5, 22.5, 30., 30.], [0., 1., 0., 0.8])],
            [0., 0., 0., 1.],
        )
        .unwrap();
        assert_eq!(
            p.effect_cache_misses,
            misses + 1,
            "fractional origin changes AA coverage"
        );
        for width in 20..65 {
            p.rasterize(
                &[make([10., 10., width as f32, 30.], [1.; 4])],
                [0., 0., 0., 1.],
            )
            .unwrap();
        }
        assert_eq!(p.effect_masks.len(), EFFECT_CACHE_ENTRIES);
        assert!(p.effect_bytes <= EFFECT_CACHE_BYTES);
        assert!(p.effect_tint_bytes <= EFFECT_TINT_BYTES);
        assert_eq!(
            p.effect_tint_bytes,
            p.effect_masks
                .values()
                .filter_map(|m| m.tint.as_ref())
                .map(|(_, t)| t.data().len())
                .sum::<usize>(),
            "mask LRU eviction also releases tinted RGBA bytes"
        );
    }
    #[test]
    fn shadow_tint_cache_reuses_rgb_during_alpha_only_animation() {
        let mut p = RasterPainter::new(Viewport::new(100., 100., 1.).unwrap());
        let batch = |alpha: f32, rgb: [f32; 3]| PaintBatch {
            clip: Rect::new(0., 0., 100., 100.).unwrap(),
            shapes: vec![Shape::Shadow(
                [20., 20., 50., 50.],
                [8.; 4],
                8.,
                0.,
                [0., 0.],
                [rgb[0], rgb[1], rgb[2], alpha],
            )],
            glyphs: vec![],
        };
        for alpha in [0.1, 0.2, 0.5, 1., 0.4] {
            p.rasterize(&[batch(alpha, [0.4, 0.7, 1.])], [0., 0., 0., 1.])
                .unwrap();
        }
        assert_eq!(p.effect_cache_misses, 1);
        assert_eq!(
            p.effect_tint_misses, 1,
            "alpha-only frames must not rebuild coverage or RGB pixels"
        );
        p.rasterize(&[batch(0.4, [1., 0.7, 0.4])], [0., 0., 0., 1.])
            .unwrap();
        assert_eq!(p.effect_cache_misses, 1);
        assert_eq!(
            p.effect_tint_misses, 2,
            "RGB change rebuilds tint, not blur geometry"
        );
        assert!(p.effect_tint_bytes <= EFFECT_TINT_BYTES);
    }
    #[test]
    fn tint_spans_match_source_over_with_fractional_clip_and_transparent_groups() {
        let mut source = Pixmap::new(8, 7).unwrap();
        for (i, pixel) in source.pixels_mut().iter_mut().enumerate() {
            *pixel =
                tiny_skia::ColorU8::from_rgba(91, 173, 237, (i * 37 % 256) as u8).premultiply();
        }
        let clip = [2.25, 1.5, 9.5, 8.2];
        let mut mask = Mask::new(17, 13).unwrap();
        fill_mask(&mut mask, 17, 13, clip, 255);
        for background in [[0., 0., 0., 0.], [0.4, 0.2, 0.7, 0.3], [0.7, 0.5, 0.1, 1.]] {
            for position in [[0, 0], [3, 2], [-3, -2], [14, 10], [-30, -20]] {
                for opacity in [0, 1, 64, 128, 254, 255] {
                    let mut actual = Pixmap::new(17, 13).unwrap();
                    actual.fill(
                        tiny_skia::Color::from_rgba(
                            background[0],
                            background[1],
                            background[2],
                            background[3],
                        )
                        .unwrap(),
                    );
                    let mut oracle = actual.clone();
                    let mut grouped = actual.clone();
                    blit_tint(&mut actual, &source, &mask, position, clip, opacity);
                    let mut group = Pixmap::new(17, 13).unwrap();
                    blit_tint(&mut group, &source, &mask, position, clip, 255);
                    composite_layer(&mut grouped, &group, opacity);
                    // Bound the pattern oracle to the source bitmap. Its Pad
                    // shader can extend a negative-position edge by one pixel;
                    // a shadow blit must not draw outside the coverage image.
                    let mut bounded_mask = mask.clone();
                    for (i, value) in bounded_mask.data_mut().iter_mut().enumerate() {
                        let x = (i % 17) as i32;
                        let y = (i / 17) as i32;
                        if x < position[0]
                            || x >= position[0] + source.width() as i32
                            || y < position[1]
                            || y >= position[1] + source.height() as i32
                        {
                            *value = 0;
                        }
                    }
                    oracle.draw_pixmap(
                        position[0],
                        position[1],
                        source.as_ref(),
                        &tiny_skia::PixmapPaint {
                            opacity: f32::from(opacity) / 255.,
                            ..Default::default()
                        },
                        Transform::identity(),
                        Some(&bounded_mask),
                    );
                    assert!(actual.data().iter().zip(oracle.data()).all(|(a, b)| a.abs_diff(*b) <= 1),
                        "source-over mismatch at {position:?}, opacity={opacity}, background={background:?}: {:?}",
                        actual.pixels().iter().zip(oracle.pixels()).enumerate().filter(|(_, (a,b))|
                            [a.red().abs_diff(b.red()),a.green().abs_diff(b.green()),a.blue().abs_diff(b.blue()),a.alpha().abs_diff(b.alpha())].into_iter().any(|d|d>1)).take(5).collect::<Vec<_>>());
                    assert!(actual.pixels().iter().all(|p| p.red() <= p.alpha()
                        && p.green() <= p.alpha()
                        && p.blue() <= p.alpha()));
                    assert!(grouped.data().iter().zip(oracle.data()).all(|(a,b)|a.abs_diff(*b)<=1),
                        "group composite mismatch at {position:?}, opacity={opacity}, background={background:?}");
                }
            }
        }
    }
    #[test]
    fn effects_respect_ancestor_clip_and_report_oversized_work() {
        let mut p = RasterPainter::new(Viewport::new(100., 100., 1.).unwrap());
        let batch = |shape| PaintBatch {
            clip: Rect::new(20., 0., 60., 100.).unwrap(),
            shapes: vec![shape],
            glyphs: vec![],
        };
        let image = p
            .rasterize(
                &[batch(Shape::Shadow(
                    [25., 25., 50., 50.],
                    [0.; 4],
                    12.,
                    0.,
                    [0., 0.],
                    [1., 0., 0., 1.],
                ))],
                [0., 0., 0., 1.],
            )
            .unwrap();
        assert!(
            image.pixels()[50 * 100 + 21].red() > 0,
            "shadow may extend beyond node bounds"
        );
        assert_eq!(
            image.pixels()[50 * 100 + 19].red(),
            0,
            "ancestor clip still limits effects"
        );
        for shape in [
            Shape::Shadow([0., 0., 3000., 3000.], [0.; 4], 1., 0., [0., 0.], [1.; 4]),
            Shape::Shadow([0., 0., f32::MAX, 30.], [0.; 4], 1., 0., [0., 0.], [1.; 4]),
            Shape::Shadow([0., 0., 30., 30.], [0.; 4], f32::NAN, 0., [0., 0.], [1.; 4]),
        ] {
            assert!(p.rasterize(&[batch(shape)], [0., 0., 0., 1.]).is_err());
        }
        p.set_viewport(Viewport::new(2000., 2000., 1.).unwrap());
        let layer_batch = PaintBatch {
            clip: Rect::new(0., 0., 2000., 2000.).unwrap(),
            shapes: vec![Shape::LayerBegin(0.5); 4],
            glyphs: vec![],
        };
        assert!(p
            .rasterize(&[layer_batch], [0., 0., 0., 1.])
            .unwrap_err()
            .contains("memory budget"));
    }
    #[test]
    fn layer_opacity_applies_to_glyphs_without_opaque_pixel_artifacts() {
        let mut p = RasterPainter::new(Viewport::new(100., 60., 1.).unwrap());
        let clip = Rect::new(0., 0., 100., 60.).unwrap();
        let mut glyphs = vec![];
        p.ui_text(&mut glyphs, "MMMM", [10., 5., 90., 55.], 30., [1.; 4]);
        let plain = p
            .rasterize(
                &[PaintBatch {
                    clip,
                    shapes: vec![],
                    glyphs: glyphs.clone(),
                }],
                [0., 0., 0., 1.],
            )
            .unwrap();
        let layered = p
            .rasterize(
                &[
                    PaintBatch {
                        clip,
                        shapes: vec![Shape::LayerBegin(0.5)],
                        glyphs,
                    },
                    PaintBatch {
                        clip,
                        shapes: vec![Shape::LayerEnd],
                        glyphs: vec![],
                    },
                ],
                [0., 0., 0., 1.],
            )
            .unwrap();
        assert!(plain.pixels().iter().any(|p| p.red() > 200));
        for (before, after) in plain.pixels().iter().zip(layered.pixels()) {
            assert!(
                after
                    .red()
                    .abs_diff((before.red() as f32 * 0.5).round() as u8)
                    <= 1
            );
        }
    }
    #[test]
    fn reusable_opacity_layers_clear_pixels_and_resize_clears_pool() {
        let mut p = RasterPainter::new(Viewport::new(60., 60., 1.).unwrap());
        let batch = |rect| PaintBatch {
            clip: Rect::new(0., 0., 60., 60.).unwrap(),
            shapes: vec![
                Shape::LayerBegin(0.5),
                Shape::Rounded(rect, 8., [1., 0., 0., 1.]),
                Shape::LayerEnd,
            ],
            glyphs: vec![],
        };
        p.rasterize(&[batch([0., 0., 30., 30.])], [0., 0., 0., 1.])
            .unwrap();
        assert_eq!(p.layer_allocations, 1);
        let next = p
            .rasterize(&[batch([30., 30., 30., 30.])], [0., 0., 0., 1.])
            .unwrap();
        assert_eq!(
            p.layer_allocations, 1,
            "animation frames reuse transparent buffers"
        );
        assert_eq!(
            next.pixels()[15 * 60 + 15].red(),
            0,
            "previous frame pixels must be cleared"
        );
        assert!(next.pixels()[45 * 60 + 45].red().abs_diff(128) <= 1);
        p.set_viewport(Viewport::new(80., 60., 1.).unwrap());
        assert!(p.layer_pool.is_empty());
    }
    #[test]
    fn equal_corner_paths_preserve_uniform_geometry() {
        let a = rounded([5.25, 8.5, 70.75, 40.5], 12.).unwrap();
        let b = rounded_corners([5.25, 8.5, 70.75, 40.5], [12.; 4]).unwrap();
        assert_eq!(a, b);
    }
    #[test]
    fn fast_rounded_fill_matches_original_path_exactly() {
        for scale in [1., 1.5, 2.] {
            for alpha in [0., 0.18, 0.5, 1.] {
                for fractional in [0., 0.25, 0.7] {
                    let mut p = RasterPainter::new(Viewport::new(280., 220., scale).unwrap());
                    let bounds = [
                        (10. + fractional) * scale,
                        (12. + fractional) * scale,
                        112.3 * scale,
                        84.7 * scale,
                    ];
                    let radius = 12.4 * scale;
                    let color = [0.13, 0.75, 0.38, alpha];
                    let clip =
                        Rect::new(9.3 * scale, 13.2 * scale, 96.8 * scale, 74.3 * scale).unwrap();
                    let fast = p
                        .rasterize(
                            &[PaintBatch {
                                clip,
                                shapes: vec![Shape::Rounded(bounds, radius, color)],
                                glyphs: vec![],
                            }],
                            [0.09, 0.1, 0.11, 1.],
                        )
                        .unwrap();
                    let mut oracle = Pixmap::new(280, 220).unwrap();
                    oracle.fill(tiny_skia::Color::from_rgba(0.09, 0.1, 0.11, 1.).unwrap());
                    let mut mask = Mask::new(280, 220).unwrap();
                    fill_mask(&mut mask, 280, 220, clip.bounds(), 255);
                    oracle.fill_path(
                        &rounded(bounds, radius).unwrap(),
                        &shape_paint(color),
                        FillRule::Winding,
                        Transform::identity(),
                        Some(&mask),
                    );
                    let first = fast
                        .pixels()
                        .iter()
                        .zip(oracle.pixels())
                        .position(|(a, b)| a != b);
                    assert!(
                        first.is_none(),
                        "scale={scale} alpha={alpha} fractional={fractional} first={first:?} pixels={:?}", first.map(|i| (fast.pixels()[i], oracle.pixels()[i]))
                    );
                }
            }
        }
    }
    #[test]
    fn font_attributes_are_cached_and_match_drawing() {
        let mut p = RasterPainter::new(Viewport::new(300., 100., 1.).unwrap());
        let base = TextStyle {
            family: "sans-serif".into(),
            size: 16.,
            ..TextStyle::default()
        };
        let bold = TextStyle {
            weight: 700,
            ..base.clone()
        };
        let italic = TextStyle {
            italic: true,
            ..base.clone()
        };
        let mono = TextStyle {
            family: "monospace".into(),
            ..base.clone()
        };
        for style in [&base, &bold, &italic, &mono] {
            let measured = p.measure_styled("Window typography", style, 280.);
            assert!(measured[0] > 0. && measured[1] > 0.);
        }
        assert_eq!(
            p.text.len(),
            4,
            "family/weight/italic need independent shaping keys"
        );
        let misses = p.text_cache_misses;
        let mut glyphs = vec![];
        p.ui_text_styled(
            &mut glyphs,
            "Window typography",
            [0., 0., 280., 100.],
            &bold,
            [1.; 4],
        );
        p.rasterize(
            &[PaintBatch {
                clip: Rect::new(0., 0., 300., 100.).unwrap(),
                shapes: vec![],
                glyphs,
            }],
            [0., 0., 0., 1.],
        )
        .unwrap();
        assert_eq!(
            p.text_cache_misses, misses,
            "draw must use the measured font attributes"
        );
    }
    #[test]
    fn native_panel_border_is_one_stroke_not_seventy_two_triangles() {
        let mut p = RasterPainter::new(Viewport::new(120., 100., 1.).unwrap());
        let mut shapes = vec![];
        let panel = crate::Panel::new(
            Rect::new(10., 10., 100., 70.).unwrap(),
            12.,
            1.,
            crate::SurfaceStyle::from_theme(&crate::Theme::dark()),
        )
        .unwrap();
        panel.paint(&p, &mut shapes);
        assert!(shapes.iter().all(|s| !matches!(s, Shape::Triangle(..))));
        let frame = p
            .rasterize(
                &[PaintBatch {
                    clip: Rect::new(0., 0., 120., 100.).unwrap(),
                    shapes,
                    glyphs: vec![],
                }],
                [0., 0., 0., 1.],
            )
            .unwrap();
        assert_eq!(
            p.shape_draw_calls, 4,
            "shadow, fill, batched border, highlight"
        );
        assert!(frame.pixels()[10 * 120 + 60].red() > 0);
        assert!(frame.pixels()[50 * 120 + 60].red() > 0);
    }
    #[test]
    fn stroke_batching_respects_order_style_and_clip() {
        let mut p = RasterPainter::new(Viewport::new(50., 50., 1.).unwrap());
        let shapes = vec![
            Shape::Line([5., 10.], [45., 10.], 4., [1., 0., 0., 1.]),
            Shape::Line([5., 20.], [45., 20.], 4., [1., 0., 0., 1.]),
            Shape::Rectangle([10., 8., 10., 5.], [0., 1., 0., 1.]),
            Shape::Line([5., 30.], [45., 30.], 4., [0., 0., 1., 1.]),
        ];
        let frame = p
            .rasterize(
                &[PaintBatch {
                    clip: Rect::new(0., 0., 35., 50.).unwrap(),
                    shapes,
                    glyphs: vec![],
                }],
                [0., 0., 0., 1.],
            )
            .unwrap();
        assert_eq!(p.shape_draw_calls, 3);
        assert_eq!(
            frame.pixels()[10 * 50 + 15].green(),
            255,
            "rectangle after stroke stays above it"
        );
        assert_eq!(frame.pixels()[20 * 50 + 25].red(), 255);
        assert_eq!(frame.pixels()[30 * 50 + 25].blue(), 255);
        assert_eq!(
            frame.pixels()[20 * 50 + 40].red(),
            0,
            "batched stroke stays clipped"
        );
    }
    #[test]
    fn effective_text_size_and_glyph_cache_budget() {
        let mut p = RasterPainter::new(Viewport::new(100., 100., 1.).unwrap());
        p.measure("x", f32::MIN_POSITIVE, 100.);
        p.measure("x", f32::MAX, 100.);
        assert!(p
            .text
            .keys()
            .all(|key| (1. ..=384.).contains(&f32::from_bits(key.size))));
        p.glyph_bytes = 17 * 1024 * 1024;
        let glyph = Glyph {
            text: "x".into(),
            bounds: [0., 0., 100., 100.],
            style: default_style(12.),
            color: [1.; 4],
        };
        p.rasterize(
            &[PaintBatch {
                clip: Rect::new(0., 0., 100., 100.).unwrap(),
                shapes: vec![],
                glyphs: vec![glyph],
            }],
            [0., 0., 0., 1.],
        )
        .unwrap();
        assert!(p.glyph_bytes < 1024 * 1024);
    }
}

#[cfg(test)]
mod generic_font_tests {
    use super::*;
    #[test]
    fn generic_family_selection_uses_installed_fonts_when_available() {
        let mut fonts = FontSystem::new();
        configure_generic_families(&mut fonts);
        for (family, candidates) in [
            (
                Family::Serif,
                &["Georgia", "Times New Roman", "DejaVu Serif", "Noto Serif"][..],
            ),
            (
                Family::SansSerif,
                &[
                    "Avenir Next",
                    "DejaVu Sans",
                    "Arial",
                    "Noto Sans",
                    "Open Sans",
                ][..],
            ),
            (
                Family::Monospace,
                &[
                    "Menlo",
                    "Consolas",
                    "DejaVu Sans Mono",
                    "Noto Sans Mono",
                    "Courier New",
                ][..],
            ),
        ] {
            if let Some(expected) = candidates.iter().find(|candidate| {
                fonts
                    .db()
                    .faces()
                    .any(|face| face.families.iter().any(|(name, _)| name == *candidate))
            }) {
                let selected = fonts
                    .db()
                    .query(&cosmic_text::fontdb::Query {
                        families: &[family],
                        ..Default::default()
                    })
                    .unwrap();
                let face = fonts.db().face(selected).unwrap();
                assert!(face.families.iter().any(|(name, _)| name == expected));
            }
        }
    }
}
