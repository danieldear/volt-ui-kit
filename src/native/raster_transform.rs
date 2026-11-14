//! Isolated affine compositing; untransformed frames never enter this path.
use super::*;
use crate::motion::Affine2D;

pub(super) fn group(
    painter: &mut RasterPainter,
    batches: Vec<PaintBatch<Shape, Glyph>>,
    matrix: Affine2D,
    ancestor: crate::Rect,
) -> Vec<PaintBatch<Shape, Glyph>> {
    if matrix.inverse().is_none() {
        return vec![];
    }
    let mut area = None;
    for batch in &batches {
        for shape in &batch.shapes {
            if let Some(r) = shape_extent(shape) {
                area = union(area, r);
            }
        }
        for g in &batch.glyphs {
            let size = painter.measure_styled(&g.text, &g.style, g.bounds[2]);
            area = union(
                area,
                [
                    g.bounds[0],
                    g.bounds[1],
                    g.bounds[2].min(size[0] + 2.),
                    g.bounds[3].min(size[1] + 2.),
                ],
            );
        }
    }
    let Some([x, y, w, h]) = area else {
        return vec![];
    };
    let (right, bottom) = ((x + w + 2.).ceil(), (y + h + 2.).ceil());
    let (x, y) = ((x - 2.).floor(), (y - 2.).floor());
    let size = [(right - x).max(1.) as u32, (bottom - y).max(1.) as u32];
    let clip = crate::Rect::new(0., 0., size[0] as f32, size[1] as f32).expect("positive size");
    let content = batches
        .iter()
        .map(|b| {
            let mut b = translated_batch(b, -x, -y);
            if b.clip
                == crate::Rect::new(
                    ancestor.bounds()[0] - x,
                    ancestor.bounds()[1] - y,
                    ancestor.bounds()[2],
                    ancestor.bounds()[3],
                )
                .unwrap()
            {
                b.clip = clip;
            }
            b
        })
        .collect();
    let matrix = Affine2D([1., 0., 0., 1., f64::from(x), f64::from(y)]).then(matrix);
    vec![PaintBatch {
        clip: ancestor,
        shapes: vec![Shape::Transformed(
            matrix,
            std::sync::Arc::new(content),
            size,
        )],
        glyphs: vec![],
    }]
}

fn scaled_batch(batch: &PaintBatch<Shape, Glyph>, s: f32) -> PaintBatch<Shape, Glyph> {
    let mut b = batch.clone();
    let scale_bounds = |r: &mut [f32; 4]| r.iter_mut().for_each(|v| *v *= s);
    let scale_point = |p: &mut [f32; 2]| p.iter_mut().for_each(|v| *v *= s);
    let scale_region = |r: &mut ClipRegion| match r {
        ClipRegion::Rounded { bounds, radii } => {
            scale_bounds(bounds);
            radii.iter_mut().for_each(|v| *v *= s);
        }
        ClipRegion::Circle { bounds } => scale_bounds(bounds),
        ClipRegion::Path(p) => *p = std::mem::take(p).transformed([s, 0., 0., s, 0., 0.]),
    };
    let [x, y, w, h] = b.clip.bounds();
    b.clip = crate::Rect::new(x * s, y * s, w * s, h * s).unwrap();
    for shape in &mut b.shapes {
        match shape {
            Shape::Transformed(m, content, size) => {
                m.0[4] *= f64::from(s);
                m.0[5] *= f64::from(s);
                *content =
                    std::sync::Arc::new(content.iter().map(|b| scaled_batch(b, s)).collect());
                *size = size.map(|v| (v as f32 * s).ceil() as u32);
            }
            Shape::Rectangle(r, _) => scale_bounds(r),
            Shape::Triangle(p, _) => p.iter_mut().for_each(scale_point),
            Shape::Rounded(r, rad, _) => {
                scale_bounds(r);
                *rad *= s;
            }
            Shape::Corners(r, rad, _) => {
                scale_bounds(r);
                rad.iter_mut().for_each(|v| *v *= s);
            }
            Shape::Outline(r, rad, width, _) => {
                scale_bounds(r);
                rad.iter_mut().for_each(|v| *v *= s);
                *width *= s;
            }
            Shape::Shadow(r, rad, blur, spread, offset, _)
            | Shape::InnerShadow(r, rad, blur, spread, offset, _) => {
                scale_bounds(r);
                rad.iter_mut().for_each(|v| *v *= s);
                *blur *= s;
                *spread *= s;
                scale_point(offset);
            }
            Shape::Line(a, b, w, _) => {
                scale_point(a);
                scale_point(b);
                *w *= s;
            }
            Shape::ClipBegin(r) => scale_region(r),
            Shape::Path(p, _, width) => {
                *p = std::mem::take(p).transformed([s, 0., 0., s, 0., 0.]);
                if let Some(w) = width {
                    *w *= s;
                }
            }
            Shape::Image(r, _, scale) => {
                scale_bounds(r);
                *scale *= s;
            }
            Shape::Gradient(r, rad, geometry, _) => {
                scale_bounds(r);
                rad.iter_mut().for_each(|v| *v *= s);
                match geometry {
                    GradientGeometry::Linear { start, end } => {
                        scale_point(start);
                        scale_point(end);
                    }
                    GradientGeometry::Radial { centre, radius } => {
                        scale_point(centre);
                        *radius *= s;
                    }
                }
            }
            _ => {}
        }
    }
    for g in &mut b.glyphs {
        scale_bounds(&mut g.bounds);
        g.style.size *= s;
    }
    b
}
impl RasterPainter {
    pub(super) fn draw_transformed(
        &mut self,
        dest: &mut Pixmap,
        mask: &Mask,
        matrix: Affine2D,
        content: &[PaintBatch<Shape, Glyph>],
        size: [u32; 2],
    ) -> Result<(), String> {
        if self.transform_depth >= 24 || matrix.inverse().is_none() {
            return Err("invalid transform or transform depth".into());
        }
        // Largest singular value: never enlarge a low-resolution cached text
        // bitmap. Rasterize at sufficient density, then filter down/rotate.
        let [a, b, c, d, _, _] = matrix.0;
        let q = a * a + b * b + c * c + d * d;
        let det = a * d - b * c;
        let density = ((q + (q * q - 4. * det * det).max(0.).sqrt()) * 0.5)
            .sqrt()
            .max(1.) as f32;
        let w = (size[0] as f32 * density).ceil() as u32;
        let h = (size[1] as f32 * density).ceil() as u32;
        if !density.is_finite()
            || w > 16384
            || h > 16384
            || u64::from(w) * u64::from(h) > 16_777_216
            || self
                .transform_bytes
                .saturating_add(u64::from(w) * u64::from(h) * 6)
                > 64 * 1024 * 1024
        {
            return Err("transformed software layer exceeds 16 megapixel budget".into());
        }
        let scaled = content
            .iter()
            .map(|b| scaled_batch(b, density))
            .collect::<Vec<_>>();
        let next_viewport = Viewport::new(w as f32, h as f32, self.viewport.scale * density)
            .ok_or("invalid transform viewport")?;
        let old_viewport = self.viewport;
        let old_damage = self.damage;
        let old_origin = self.raster_origin;
        let old_pool = std::mem::take(&mut self.layer_pool);
        let old_masks = std::mem::take(&mut self.clip_masks);
        let old_bytes = std::mem::take(&mut self.clip_bytes);
        self.viewport = next_viewport;
        self.damage = None;
        self.raster_origin = [0; 2];
        self.transform_depth += 1;
        self.transform_bytes += u64::from(w) * u64::from(h) * 6;
        let result = self.rasterize_into(&scaled, [0.; 4], None);
        self.transform_depth -= 1;
        self.transform_bytes -= u64::from(w) * u64::from(h) * 6;
        self.viewport = old_viewport;
        self.damage = old_damage;
        self.raster_origin = old_origin;
        self.layer_pool = old_pool;
        self.clip_masks = old_masks;
        self.clip_bytes = old_bytes;
        let source = result?;
        let [a, b, c, d, x, y] = matrix.0;
        // tiny-skia 0.11 forces nearest sampling for *all* pure translations,
        // including fractional ones. Preserve subpixel drift rather than
        // snapping this composited text/card back onto whole pixels.
        if [a, b, c, d] == [1., 0., 0., 1.] && (x.fract() != 0. || y.fract() != 0.) {
            fractional_translation(dest, mask, &source, [x, y]);
            return Ok(());
        }
        dest.draw_pixmap(
            0,
            0,
            source.as_ref(),
            &tiny_skia::PixmapPaint {
                quality: tiny_skia::FilterQuality::Bicubic,
                ..Default::default()
            },
            Transform::from_row(
                a as f32 / density,
                b as f32 / density,
                c as f32 / density,
                d as f32 / density,
                x as f32,
                y as f32,
            ),
            Some(mask),
        );
        Ok(())
    }
}

/// Four-tap premultiplied translation. No extra staging image; only the
/// destination overlap is visited and the existing rounded/path mask applies.
fn fractional_translation(dest: &mut Pixmap, mask: &Mask, source: &Pixmap, [x, y]: [f64; 2]) {
    let (ox, oy) = (x.floor() as i64, y.floor() as i64);
    let (fx, fy) = ((x - x.floor()) as f32, (y - y.floor()) as f32);
    let taps = [
        (0, 0, (1. - fx) * (1. - fy)),
        (-1, 0, fx * (1. - fy)),
        (0, -1, (1. - fx) * fy),
        (-1, -1, fx * fy),
    ];
    let (sw, sh) = (i64::from(source.width()), i64::from(source.height()));
    let (dw, dh) = (i64::from(dest.width()), i64::from(dest.height()));
    let left = ox.clamp(0, dw);
    let top = oy.clamp(0, dh);
    let right = ox.saturating_add(sw).saturating_add(1).clamp(left, dw);
    let bottom = oy.saturating_add(sh).saturating_add(1).clamp(top, dh);
    for dy in top..bottom {
        for dx in left..right {
            let index = (dy * dw + dx) as usize;
            let coverage = u32::from(mask.data()[index]);
            if coverage == 0 {
                continue;
            }
            let mut rgba = [0.; 4];
            for (tx, ty, weight) in taps {
                let (sx, sy) = (dx - ox + tx, dy - oy + ty);
                if sx >= 0 && sy >= 0 && sx < sw && sy < sh && weight > 0. {
                    let i = (sy * sw + sx) as usize * 4;
                    for (channel, value) in rgba.iter_mut().enumerate() {
                        *value += f32::from(source.data()[i + channel]) * weight;
                    }
                }
            }
            let rgba = rgba.map(|v| v.round().clamp(0., 255.) as u8);
            if rgba[3] != 0 {
                blend_premultiplied(
                    &mut dest.data_mut()[index * 4..index * 4 + 4],
                    &rgba,
                    &|v| ((u32::from(v) * coverage + 127) / 255) as u8,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fractional_translation_preserves_coverage_mask_and_premultiplication() {
        let mut source = Pixmap::new(1, 1).unwrap();
        source.fill(tiny_skia::Color::from_rgba8(255, 0, 0, 255));
        let mut target = Pixmap::new(3, 2).unwrap();
        let mut mask = Mask::new(3, 2).unwrap();
        mask.data_mut().fill(255);
        mask.data_mut()[1] = 0;
        fractional_translation(&mut target, &mask, &source, [0.25, 0.5]);
        assert_eq!(target.pixel(0, 0).unwrap().alpha(), 96);
        assert_eq!(target.pixel(1, 0).unwrap().alpha(), 0);
        assert_eq!(target.pixel(0, 1).unwrap().alpha(), 96);
        assert_eq!(target.pixel(1, 1).unwrap().alpha(), 32);
        assert!(target
            .pixels()
            .iter()
            .all(|p| p.red() == p.alpha() && p.green() == 0 && p.blue() == 0));
        let mut clipped = Pixmap::new(3, 2).unwrap();
        fractional_translation(&mut clipped, &mask, &source, [-0.5, -0.5]);
        assert_eq!(clipped.pixel(0, 0).unwrap().alpha(), 64);
        assert!(clipped.pixels()[1..].iter().all(|p| p.alpha() == 0));
    }
}
