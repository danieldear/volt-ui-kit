//! Paint text presets with shaped glyphs and clips; never raster-cache a title image.
use super::super::motion_text::TextPaint;
use super::*;
use unicode_segmentation::UnicodeSegmentation;
impl Runtime {
    pub(super) fn paint_moving_text<P: DocumentPainter>(
        &self,
        p: &mut P,
        ui: &UiTheme,
        b: &LayoutBox,
        visual: Option<Visual>,
        out: &mut Vec<PaintBatch<P::Shape, P::Glyph>>,
    ) -> bool {
        let Some(effect) = &b.text_paint else {
            return false;
        };
        let mut base = b.clone();
        base.text.clear();
        base.text_paint = None;
        out.push(self.paint_one(p, ui, &base, visual));
        let r = b.bounds.bounds();
        let color = b.style.color.unwrap_or(ui.colors().text);
        let mut glyphs = Vec::new();
        match effect {
            TextPaint::CrossFade { from, progress } => {
                let mut old = color;
                old[3] *= 1. - progress;
                let mut new = color;
                new[3] *= progress;
                p.ui_text_styled(&mut glyphs, from, r, &b.text_style, old);
                p.ui_text_styled(&mut glyphs, &b.text, r, &b.text_style, new);
                out.push(PaintBatch {
                    clip: b.clip,
                    shapes: vec![],
                    glyphs,
                });
            }
            TextPaint::Rolling { from, progress } => {
                let mut shapes = vec![];
                p.ui_clip_begin(
                    &mut shapes,
                    ClipRegion::Rounded {
                        bounds: r,
                        radii: [0.; 4],
                    },
                );
                out.push(PaintBatch {
                    clip: b.clip,
                    shapes,
                    glyphs: vec![],
                });
                let width = p.measure_styled("0", &b.text_style, 16384.)[0].max(1.);
                let old = from.chars().collect::<Vec<_>>();
                let new = b.text.chars().collect::<Vec<_>>();
                let count = old.len().max(new.len());
                for i in 0..count {
                    let a = i.checked_sub(count - old.len()).and_then(|i| old.get(i));
                    let z = i.checked_sub(count - new.len()).and_then(|i| new.get(i));
                    let x = r[0] + r[2] - width * (count - i) as f32;
                    let mut draw = |c: char, dy: f32| {
                        let mut glyphs = vec![];
                        p.ui_text_styled(
                            &mut glyphs,
                            &c.to_string(),
                            [x, r[1], width, r[3]],
                            &b.text_style,
                            color,
                        );
                        let batch = PaintBatch {
                            clip: Rect::new(r[0], r[1], r[2], r[3]).unwrap(),
                            shapes: vec![],
                            glyphs,
                        };
                        // Fractional movement belongs in the scene transform,
                        // not in pixel-snapped glyph origins.
                        out.extend(p.ui_transform_batches(
                            vec![batch],
                            crate::motion::Affine2D([1., 0., 0., 1., 0., f64::from(dy)]),
                            Rect::new(r[0], r[1], r[2], r[3]).unwrap(),
                        ));
                    };
                    if a == z {
                        if let Some(c) = z {
                            draw(*c, 0.);
                        }
                    } else {
                        if let Some(c) = a {
                            draw(*c, -r[3] * progress);
                        }
                        if let Some(c) = z {
                            draw(*c, r[3] * (1. - progress));
                        }
                    }
                }
                let mut shapes = vec![];
                p.ui_clip_end(&mut shapes);
                out.push(PaintBatch {
                    clip: b.clip,
                    shapes,
                    glyphs: vec![],
                });
            }
            TextPaint::Shimmer(phase) => {
                let mut style = b.text_style.clone();
                let graphemes = b.text.grapheme_indices(true).collect::<Vec<_>>();
                for (i, (start, g)) in graphemes.iter().enumerate() {
                    let distance =
                        (i as f32 / graphemes.len().max(1) as f32 - (-0.3 + phase * 1.6)).abs();
                    let k = (1. - distance / 0.3).clamp(0., 1.);
                    let mut c = color;
                    c[3] *= 0.4 + 0.6 * k;
                    style.runs.push(TextRun {
                        start: *start,
                        end: start + g.len(),
                        color: Some(c),
                        ..Default::default()
                    });
                }
                p.ui_text_styled(&mut glyphs, &b.text, r, &style, color);
                out.push(PaintBatch {
                    clip: b.clip,
                    shapes: vec![],
                    glyphs,
                });
            }
        }
        true
    }
}
