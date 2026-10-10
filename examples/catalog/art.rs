//! Generated artwork and line icons; nothing is downloaded or bundled.
use super::App;
use volt_ui_kit::ui::{prelude::*, Image};

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0., 1.);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// A soft dusk landscape: gradient sky, sun, two ridgelines and still water.
pub fn landscape(width: u32, height: u32) -> Image {
    let sky = ([0.20, 0.25, 0.45], [0.95, 0.62, 0.48]);
    let ridge = |u: f32, k: f32, seed: f32| {
        0.55 - k * (0.35 * (u * 7. + seed).sin().abs() + 0.15 * (u * 19. + seed * 2.).sin())
    };
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let (u, v) = (x as f32 / width as f32, y as f32 / height as f32);
            let horizon = 0.66;
            let sample = |v: f32| {
                let mut c = mix(sky.0, sky.1, v / horizon);
                let d = ((u - 0.68) * 1.6).hypot(v - 0.5);
                c = mix(c, [1., 0.9, 0.75], 1. - (d / 0.07).clamp(0., 1.));
                if v > ridge(u, 0.35, 1.3) {
                    c = mix(c, [0.36, 0.30, 0.45], 0.85);
                }
                if v > ridge(u, 0.22, 4.1) + 0.08 {
                    c = [0.17, 0.15, 0.26];
                }
                c
            };
            let c = if v > horizon {
                let mirrored = sample(2. * horizon - v + 0.004 * (v * 300.).sin());
                mix(mirrored, [0.12, 0.13, 0.25], 0.35 + (v - horizon))
            } else {
                sample(v)
            };
            pixels.extend(c.map(|v| (v.clamp(0., 1.) * 255.) as u8));
            pixels.push(255);
        }
    }
    Image::from_rgba(width, height, pixels).expect("bounded generated image")
}

/// Line icons on a 24-unit grid, scaled to `size` logical pixels.
#[derive(Clone, Copy)]
pub enum Icon {
    Search,
    Sun,
    Moon,
    Check,
    Bell,
    Spark,
}
pub fn icon(glyph: Icon, size: f32, color: impl Into<String>) -> Element<App> {
    let color = color.into();
    let s = size / 24.;
    let m = [s, 0., 0., s, 0., 0.];
    canvas(move |c| {
        let p = VectorPath::new;
        let stroke = |c: &mut Canvas, path: VectorPath| {
            c.stroke(path.transformed(m), color.clone(), 1.8 * s)
        };
        match glyph {
            Icon::Search => {
                stroke(c, VectorPath::circle(10.5, 10.5, 6.5));
                stroke(c, p().move_to(15.5, 15.5).line_to(20.5, 20.5));
            }
            Icon::Sun => {
                stroke(c, VectorPath::circle(12., 12., 4.));
                for i in 0..8 {
                    let (sn, cs) = (i as f32 * std::f32::consts::FRAC_PI_4).sin_cos();
                    stroke(
                        c,
                        p().move_to(12. + cs * 7., 12. + sn * 7.)
                            .line_to(12. + cs * 9.5, 12. + sn * 9.5),
                    );
                }
            }
            Icon::Moon => stroke(
                c,
                p().move_to(15., 4.)
                    .cubic_to([9., 4.5], [5.5, 9.], [6., 13.5])
                    .cubic_to([6.6, 18.], [11., 20.5], [15.5, 19.5])
                    .cubic_to([17.6, 19.], [19., 18.], [20., 16.5])
                    .cubic_to([14., 17.5], [10., 12.], [15., 4.])
                    .close(),
            ),
            Icon::Check => stroke(c, p().move_to(5., 12.5).line_to(10., 17.).line_to(19., 7.)),
            Icon::Bell => {
                stroke(
                    c,
                    p().move_to(6., 17.)
                        .line_to(6., 11.)
                        .cubic_to([6., 7.5], [8.7, 5.], [12., 5.])
                        .cubic_to([15.3, 5.], [18., 7.5], [18., 11.])
                        .line_to(18., 17.)
                        .close(),
                );
                stroke(c, p().move_to(10., 20.).line_to(14., 20.));
            }
            Icon::Spark => {
                c.fill(
                    p().move_to(12., 3.)
                        .line_to(14., 10.)
                        .line_to(21., 12.)
                        .line_to(14., 14.)
                        .line_to(12., 21.)
                        .line_to(10., 14.)
                        .line_to(3., 12.)
                        .line_to(10., 10.)
                        .close()
                        .transformed(m),
                    color.clone(),
                );
            }
        }
    })
    .width(size)
    .height(size)
}

/// The catalog mark: a rounded square holding a lightning stroke.
pub fn mark(size: f32) -> Element<App> {
    let s = size / 24.;
    let m = [s, 0., 0., s, 0., 0.];
    canvas(move |c| {
        let square = VectorPath::new()
            .move_to(6., 0.)
            .line_to(18., 0.)
            .cubic_to([21.3, 0.], [24., 2.7], [24., 6.])
            .line_to(24., 18.)
            .cubic_to([24., 21.3], [21.3, 24.], [18., 24.])
            .line_to(6., 24.)
            .cubic_to([2.7, 24.], [0., 21.3], [0., 18.])
            .line_to(0., 6.)
            .cubic_to([0., 2.7], [2.7, 0.], [6., 0.])
            .close();
        c.fill(square.transformed(m), ColorToken::Primary);
        c.fill(
            VectorPath::new()
                .move_to(13.5, 4.)
                .line_to(7., 13.5)
                .line_to(11.5, 13.5)
                .line_to(10.5, 20.)
                .line_to(17., 10.5)
                .line_to(12.5, 10.5)
                .close()
                .transformed(m),
            ColorToken::PrimaryContent,
        );
    })
    .width(size)
    .height(size)
}
