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
            clock: 0,
            glyph_bytes: 0,
        }
    }
    pub fn set_viewport(&mut self, viewport: Viewport) {
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
                        if fill_rounded_fast(
                            &mut pixmap,
                            &mask,
                            [x, y, cw, ch],
                            *bounds,
                            *radius,
                            *c,
                        ) {
                            self.shape_draw_calls += 1;
                            continue;
                        }
                        let Some(path) = rounded(*bounds, *radius) else {
                            continue;
                        };
                        (path, *c)
                    }
                    Shape::Line(..) => unreachable!("lines handled above"),
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
                                let blend = |v: u8, old: u8| {
                                    ((v as f32 * alpha) + (old as f32 * (1. - alpha))).round() as u8
                                };
                                if let Some(next) = tiny_skia::PremultipliedColorU8::from_rgba(
                                    blend(r, pixel.red()),
                                    blend(g, pixel.green()),
                                    blend(b, pixel.blue()),
                                    255,
                                ) {
                                    *pixel = next;
                                }
                            },
                        );
                    }
                }
            }
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
