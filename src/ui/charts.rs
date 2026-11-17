//! Data charts with axes: area, candlestick, donut and stacked bars. Drawn
//! as retained vector paths; colours follow the theme (series 1 is
//! `primary`). Sizes are logical pixels.
//!
//! Give a chart a `.key(..)` to make it interactive: every data point gets
//! an invisible, focusable hit target with a value tooltip (hover or
//! keyboard focus, no recomposition), and `.selected(..)` /
//! `.on_select(..)` turn clicks into a highlighted selection.
use super::*;
use std::{cell::RefCell, rc::Rc};

type Pick<C> = Rc<RefCell<dyn FnMut(&mut C, usize)>>;
/// Interaction settings shared by every chart.
struct Interact<C> {
    key: Option<String>,
    selected: Option<usize>,
    on_select: Option<Pick<C>>,
}
impl<C> Default for Interact<C> {
    fn default() -> Self {
        Self {
            key: None,
            selected: None,
            on_select: None,
        }
    }
}
/// Builder methods every chart shares.
macro_rules! interactive {
    ($chart:ident) => {
        impl<C> $chart<C> {
            /// Make the chart interactive: hover/focus tooltips per data point.
            pub fn key(mut self, key: impl Into<String>) -> Self {
                self.interact.key = Some(key.into());
                self
            }
            /// Highlight one data point (index into the data).
            pub fn selected(mut self, index: Option<usize>) -> Self {
                self.interact.selected = index;
                self
            }
            /// Called with a data point's index when it is clicked or
            /// activated from the keyboard. Requires `.key(..)`.
            pub fn on_select(mut self, f: impl FnMut(&mut C, usize) + 'static) -> Self {
                self.interact.on_select = Some(Rc::new(RefCell::new(f)));
                self
            }
        }
    };
}
/// One invisible hit target: a focusable button with a value tooltip,
/// tinted while hovered. `tip` lines become the accessible name too.
fn hit_target<C: 'static>(
    key: &str,
    index: usize,
    size: [f32; 2],
    tip: String,
    on_select: Option<Pick<C>>,
) -> Element<C> {
    let label = tip.replace('\n', ", ");
    let mut b = button("")
        .key(format!("{key}-{index}"))
        .accessible_label(label)
        .variant(ButtonVariant::Ghost)
        .width(size[0])
        .height(size[1])
        .rounded(0.)
        .border_width(0.)
        .background("#00000000")
        .hover_style(|s| s.background(ColorToken::BaseContent.alpha(7)))
        .cursor(if on_select.is_some() {
            Cursor::Pointer
        } else {
            Cursor::Default
        });
    b = match on_select {
        Some(f) => b.on_click(move |app: &mut C| (f.borrow_mut())(app, index)),
        None => b.on_click(|_: &mut C| {}),
    };
    tooltip(tip, b).key(format!("{key}-{index}-tip")).into()
}
/// A row of full-height hit columns over the plot.
fn hit_columns<C: 'static>(
    key: &str,
    widths: &[f32],
    height: f32,
    tips: Vec<String>,
    on_select: &Option<Pick<C>>,
) -> Element<C> {
    hstack(
        widths
            .iter()
            .zip(tips)
            .enumerate()
            .map(|(i, (w, tip))| hit_target(key, i, [*w, height], tip, on_select.clone()))
            .collect::<Vec<_>>(),
    )
    .key(format!("{key}-hits"))
    .into()
}

/// Series colours: the theme's primary, then hues that stay distinct.
const SERIES: [&str; 6] = [
    "@primary", "#0EA5E9", "#F59E0B", "#10B981", "#8B5CF6", "#EF4444",
];
fn series_color(i: usize) -> &'static str {
    SERIES[i % SERIES.len()]
}
/// A colour at a percentage opacity: tokens use `@name/NN`, hex gets an
/// alpha byte.
fn faded(color: &str, percent: u8) -> String {
    if color.starts_with('@') {
        format!("{color}/{percent}")
    } else {
        format!("{color}{:02X}", (u32::from(percent.min(100)) * 255 / 100))
    }
}

/// Up to `count` round tick values covering `lo..=hi` (1, 2, 2.5 or 5 × 10ⁿ
/// steps), plus the expanded range they span.
pub fn nice_ticks(lo: f32, hi: f32, count: usize) -> (Vec<f32>, (f32, f32)) {
    let (lo, hi) = if (hi - lo).abs() < f32::EPSILON {
        (lo - 1., hi + 1.)
    } else {
        (lo.min(hi), lo.max(hi))
    };
    let raw = (hi - lo) / count.max(1) as f32;
    let mag = 10f32.powf(raw.log10().floor());
    let step = [1., 2., 2.5, 5., 10.]
        .into_iter()
        .map(|m| m * mag)
        .find(|s| *s >= raw)
        .unwrap_or(10. * mag);
    let start = (lo / step).floor() * step;
    let end = (hi / step).ceil() * step;
    let n = ((end - start) / step).round() as usize;
    let ticks = (0..=n.min(64)).map(|i| start + i as f32 * step).collect();
    (ticks, (start, end))
}
/// Compact tick labels: 1.2k, 3.4M, trimmed decimals.
pub fn short_number(v: f32) -> String {
    let a = v.abs();
    let (scaled, suffix) = if a >= 1e9 {
        (v / 1e9, "B")
    } else if a >= 1e6 {
        (v / 1e6, "M")
    } else if a >= 1e3 {
        (v / 1e3, "k")
    } else {
        (v, "")
    };
    let text = if scaled.fract().abs() < 1e-3 {
        format!("{scaled:.0}")
    } else if scaled.abs() < 10. {
        format!("{scaled:.2}")
    } else {
        format!("{scaled:.1}")
    };
    let text = if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        text
    };
    format!("{text}{suffix}")
}

/// Packs many small shapes of one colour into as few canvas paths as possible.
/// A canvas holds at most 256 paths, so one path per candle or bar would cap a
/// chart at roughly a hundred items. Shapes in one batch must not overlap:
/// canvas fills use the even-odd rule.
pub(super) struct PathBatch(Vec<VectorPath>);
impl PathBatch {
    pub(super) fn new() -> Self {
        Self(Vec::new())
    }
    pub(super) fn add(&mut self, shape: VectorPath) {
        match self.0.last_mut() {
            Some(path)
                if path.commands.len() + shape.commands.len()
                    <= crate::graphics::MAX_PATH_COMMANDS =>
            {
                path.commands.extend(shape.commands)
            }
            _ => self.0.push(shape),
        }
    }
    pub(super) fn fill(self, c: &mut Canvas, color: impl Into<String>) {
        let color = color.into();
        for path in self.0 {
            c.fill(path, color.clone());
        }
    }
    pub(super) fn stroke(self, c: &mut Canvas, color: impl Into<String>, width: f32) {
        let color = color.into();
        for path in self.0 {
            c.stroke(path, color.clone(), width);
        }
    }
}

type Format = fn(f32) -> String;
const AXIS: f32 = 44.;
const X_LABELS: f32 = 20.;

/// A plot area with Y tick labels at the left and X labels underneath.
/// `draw` receives the plot size and the value range mapped to it.
#[allow(clippy::too_many_arguments)]
fn framed<C>(
    width: f32,
    height: f32,
    range: (f32, f32),
    ticks: Vec<f32>,
    format: Format,
    x_labels: Vec<(f32, String)>,
    overlay: Option<Element<C>>,
    draw: impl FnOnce(&mut Canvas, f32, f32, &dyn Fn(f32) -> f32),
) -> Element<C> {
    let plot_w = (width - AXIS).max(10.);
    let plot_h = (height - X_LABELS).max(10.);
    let to_y = move |v: f32| plot_h - (v - range.0) / (range.1 - range.0) * plot_h;
    let grid_ticks = ticks.clone();
    let plot = canvas(move |c| {
        for t in &grid_ticks {
            let y = to_y(*t).round() + 0.5;
            c.stroke(
                VectorPath::new().move_to(0., y).line_to(plot_w, y),
                ColorToken::Border,
                1.,
            );
        }
        draw(c, plot_w, plot_h, &to_y);
    })
    .width(plot_w)
    .height(plot_h);
    let plot: Element<C> = match overlay {
        Some(hits) => zstack![plot, hits]
            .width(plot_w)
            .height(plot_h)
            .flow(Flow::TopLeft)
            .into(),
        None => plot,
    };
    // Tick labels sit on their gridlines.
    let labels: Vec<Element<C>> = ticks
        .iter()
        .map(|t| {
            hstack![text(format(*t)).font_size(11.).color(ColorToken::Muted)]
                .width(AXIS - 8.)
                .justify(Justify::End)
                .padding((to_y(*t) - 7.).clamp(0., plot_h - 14.), Edge::Top)
                .into()
        })
        .collect();
    let axis = zstack(labels)
        .width(AXIS)
        .height(plot_h)
        .flow(Flow::TopLeft);
    let mut parts = vec![hstack![axis, plot].align(Align::Start).into()];
    if !x_labels.is_empty() {
        parts.push(x_label_row(x_labels, width));
    }
    vstack(parts).width(width).into()
}
/// Labels centred under their x positions in the plot area.
fn x_label_row<C>(x_labels: Vec<(f32, String)>, width: f32) -> Element<C> {
    let xs: Vec<Element<C>> = x_labels
        .into_iter()
        .map(|(x, label)| {
            // Centred on x, but kept inside the chart at both ends.
            let w = label.chars().count() as f32 * 6.5;
            let left = (AXIS + x - w / 2.).clamp(AXIS, (width - w - 2.).max(AXIS));
            hstack![text(label)
                .font_size(11.)
                .color(ColorToken::Muted)
                .no_wrap(true)]
            .padding(left, Edge::Left)
            .into()
        })
        .collect();
    zstack(xs)
        .height(X_LABELS)
        .flow(Flow::BottomLeft)
        .width(width)
        .into()
}
/// X positions for `n` labels across `w`, thinned so they never crowd.
fn spread_labels(labels: &[String], w: f32, centered_slots: bool) -> Vec<(f32, String)> {
    let n = labels.len();
    if n == 0 {
        return Vec::new();
    }
    // Room per label from its length (≈6.5 px per character at 11 px).
    let longest = labels.iter().map(|l| l.chars().count()).max().unwrap_or(1) as f32;
    let room = longest * 6.5 + 14.;
    let every = ((n as f32 * room) / w).ceil().max(1.) as usize;
    labels
        .iter()
        .enumerate()
        .filter(|(i, _)| i % every == 0)
        .map(|(i, l)| {
            let x = if centered_slots {
                (i as f32 + 0.5) * w / n as f32
            } else if n > 1 {
                i as f32 * w / (n - 1) as f32
            } else {
                w / 2.
            };
            (x, l.clone())
        })
        .collect()
}
fn legend<C>(names: &[String]) -> Element<C> {
    hstack(
        names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                hstack![
                    vstack![]
                        .width(10.)
                        .height(10.)
                        .rounded(3.)
                        .background(series_color(i)),
                    text(name.clone()).font_size(12.).color(ColorToken::Muted),
                ]
                .gap(6., Axis::Horizontal)
                .align(Align::Center)
                .into()
            })
            .collect::<Vec<Element<C>>>(),
    )
    .gap(14., Axis::Horizontal)
    .wrap(true)
    .into()
}

// ── Area chart ──────────────────────────────────────────────────────────

/// Filled series over shared x positions, overlaid or stacked.
pub struct AreaChart<C> {
    series: Vec<(String, Vec<f32>)>,
    size: [f32; 2],
    x_labels: Vec<String>,
    stacked: bool,
    legend: bool,
    format: Format,
    interact: Interact<C>,
}
pub fn area_chart<C>(
    series: impl IntoIterator<Item = (impl Into<String>, Vec<f32>)>,
) -> AreaChart<C> {
    AreaChart {
        series: series.into_iter().map(|(n, v)| (n.into(), v)).collect(),
        size: [480., 220.],
        x_labels: Vec::new(),
        stacked: false,
        legend: true,
        format: short_number,
        interact: Interact::default(),
    }
}
interactive!(AreaChart);
impl<C> AreaChart<C> {
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.size = [width, height];
        self
    }
    pub fn x_labels(mut self, labels: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.x_labels = labels.into_iter().map(Into::into).collect();
        self
    }
    /// Draw each series on top of the ones before it.
    pub fn stacked(mut self, value: bool) -> Self {
        self.stacked = value;
        self
    }
    pub fn legend(mut self, value: bool) -> Self {
        self.legend = value;
        self
    }
    pub fn value_format(mut self, f: fn(f32) -> String) -> Self {
        self.format = f;
        self
    }
}
impl<C: 'static> From<AreaChart<C>> for Element<C> {
    fn from(a: AreaChart<C>) -> Self {
        let n = a.series.iter().map(|(_, v)| v.len()).max().unwrap_or(0);
        let value = |s: usize, x: usize| {
            a.series[s]
                .1
                .get(x)
                .copied()
                .filter(|v| v.is_finite())
                .unwrap_or(0.)
        };
        // Cumulative tops when stacked.
        let mut tops: Vec<Vec<f32>> = Vec::new();
        for i in 0..a.series.len() {
            let row: Vec<f32> = (0..n)
                .map(|x| {
                    if a.stacked && i > 0 {
                        tops[i - 1][x] + value(i, x)
                    } else {
                        value(i, x)
                    }
                })
                .collect();
            tops.push(row);
        }
        let all: Vec<f32> = tops.iter().flatten().copied().collect();
        let lo = if a.stacked {
            0.
        } else {
            all.iter().copied().fold(f32::MAX, f32::min).min(0.)
        };
        let hi = all.iter().copied().fold(f32::MIN, f32::max);
        let (ticks, range) = nice_ticks(lo, hi.max(lo + 1.), 4);
        let legend_names: Vec<String> = a.series.iter().map(|(n, _)| n.clone()).collect();
        let height = a.size[1] - if a.legend { 26. } else { 0. };
        let plot_w = a.size[0] - AXIS;
        let plot_h = (height - X_LABELS).max(10.);
        let x_labels = spread_labels(&a.x_labels, plot_w, false);
        // One hit column per x position, centred on it.
        let overlay = a.interact.key.as_deref().filter(|_| n > 1).map(|key| {
            let dx = plot_w / (n - 1) as f32;
            let widths: Vec<f32> = (0..n)
                .map(|i| if i == 0 || i == n - 1 { dx / 2. } else { dx })
                .collect();
            let tips = (0..n)
                .map(|x| {
                    let mut tip = a
                        .x_labels
                        .get(x)
                        .cloned()
                        .unwrap_or_else(|| format!("#{}", x + 1));
                    for (s, name) in legend_names.iter().enumerate() {
                        tip.push_str(&format!("\n{name}: {}", (a.format)(value(s, x))));
                    }
                    tip
                })
                .collect();
            hit_columns(key, &widths, plot_h, tips, &a.interact.on_select)
        });
        let (stacked, selected) = (a.stacked, a.interact.selected.filter(|i| *i < n));
        let chart = framed(
            a.size[0],
            height,
            range,
            ticks,
            a.format,
            x_labels,
            overlay,
            move |c, w, h, to_y| {
                if n < 2 {
                    return;
                }
                let dx = w / (n - 1) as f32;
                // Paint later series first when overlaid so the first stays on top.
                let order: Vec<usize> = if stacked {
                    (0..tops.len()).collect()
                } else {
                    (0..tops.len()).rev().collect()
                };
                for i in order {
                    let top: Vec<[f32; 2]> =
                        (0..n).map(|x| [x as f32 * dx, to_y(tops[i][x])]).collect();
                    let base: Vec<[f32; 2]> = (0..n)
                        .rev()
                        .map(|x| {
                            let b = if stacked && i > 0 {
                                tops[i - 1][x]
                            } else {
                                range.0.max(0.)
                            };
                            [x as f32 * dx, to_y(b).min(h)]
                        })
                        .collect();
                    let mut area = VectorPath::new().move_to(top[0][0], top[0][1]);
                    for p in &top[1..] {
                        area = area.line_to(p[0], p[1]);
                    }
                    for p in &base {
                        area = area.line_to(p[0], p[1]);
                    }
                    let color = series_color(i);
                    c.fill(area.close(), faded(color, if stacked { 55 } else { 22 }));
                    let mut line = VectorPath::new().move_to(top[0][0], top[0][1]);
                    for p in &top[1..] {
                        line = line.line_to(p[0], p[1]);
                    }
                    c.stroke(line, color, 2.);
                }
                // Selection: a rule through the point and a dot per series.
                if let Some(x) = selected {
                    let px = (x as f32 * dx).round() + 0.5;
                    c.stroke(
                        VectorPath::new().move_to(px, 0.).line_to(px, h),
                        ColorToken::BaseContent.alpha(45),
                        1.,
                    );
                    for (i, row) in tops.iter().enumerate() {
                        let y = to_y(row[x]);
                        c.fill(VectorPath::circle(px, y, 5.), ColorToken::Base100);
                        c.fill(VectorPath::circle(px, y, 3.5), series_color(i));
                    }
                }
            },
        );
        if a.legend {
            vstack![chart, legend(&legend_names)]
                .gap(10., Axis::Vertical)
                .into()
        } else {
            chart
        }
    }
}

// ── Candlesticks ────────────────────────────────────────────────────────

/// One period of open/high/low/close prices and optional volume.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candle {
    pub open: f32,
    pub high: f32,
    pub low: f32,
    pub close: f32,
    pub volume: f32,
}
impl Candle {
    pub fn new(open: f32, high: f32, low: f32, close: f32) -> Self {
        Self {
            open,
            high,
            low,
            close,
            volume: 0.,
        }
    }
    pub fn volume(mut self, value: f32) -> Self {
        self.volume = value;
        self
    }
    fn valid(&self) -> bool {
        [self.open, self.high, self.low, self.close, self.volume]
            .iter()
            .all(|v| v.is_finite())
            && self.low <= self.high
    }
}
/// Price candles: rising periods in `success`, falling in `error`, with an
/// optional volume strip underneath.
pub struct CandlestickChart<C> {
    candles: Vec<Candle>,
    size: [f32; 2],
    x_labels: Vec<String>,
    volume: bool,
    format: Format,
    interact: Interact<C>,
}
pub fn candlestick_chart<C>(candles: impl IntoIterator<Item = Candle>) -> CandlestickChart<C> {
    CandlestickChart {
        candles: candles.into_iter().filter(Candle::valid).collect(),
        size: [560., 280.],
        x_labels: Vec::new(),
        volume: false,
        format: short_number,
        interact: Interact::default(),
    }
}
interactive!(CandlestickChart);
impl<C> CandlestickChart<C> {
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.size = [width, height];
        self
    }
    pub fn x_labels(mut self, labels: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.x_labels = labels.into_iter().map(Into::into).collect();
        self
    }
    /// Show volume bars in a strip below the prices.
    pub fn volume(mut self, value: bool) -> Self {
        self.volume = value;
        self
    }
    pub fn value_format(mut self, f: fn(f32) -> String) -> Self {
        self.format = f;
        self
    }
}
impl<C: 'static> From<CandlestickChart<C>> for Element<C> {
    fn from(k: CandlestickChart<C>) -> Self {
        let lo = k.candles.iter().map(|c| c.low).fold(f32::MAX, f32::min);
        let hi = k.candles.iter().map(|c| c.high).fold(f32::MIN, f32::max);
        let (ticks, range) = if k.candles.is_empty() {
            nice_ticks(0., 1., 4)
        } else {
            nice_ticks(lo, hi, 4)
        };
        let vol_h = if k.volume {
            (k.size[1] * 0.18).round()
        } else {
            0.
        };
        let plot_w = k.size[0] - AXIS;
        let height = k.size[1] - vol_h - if k.volume { X_LABELS } else { 0. };
        let plot_h = (height - X_LABELS).max(10.);
        let x_labels = spread_labels(&k.x_labels, plot_w, true);
        // With volume, the dates go under the volume strip instead.
        let (inline_labels, trailing_labels) = if k.volume {
            (Vec::new(), x_labels)
        } else {
            (x_labels, Vec::new())
        };
        let n = k.candles.len().max(1);
        let f = k.format;
        let overlay = k
            .interact
            .key
            .as_deref()
            .filter(|_| !k.candles.is_empty())
            .map(|key| {
                let tips = k
                    .candles
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        let mut tip = k
                            .x_labels
                            .get(i)
                            .cloned()
                            .unwrap_or_else(|| format!("#{}", i + 1));
                        let change = if c.open != 0. {
                            (c.close - c.open) / c.open * 100.
                        } else {
                            0.
                        };
                        tip.push_str(&format!(
                            "\nOpen {}  High {}\nLow {}  Close {}\nChange {}{change:.2}%",
                            f(c.open),
                            f(c.high),
                            f(c.low),
                            f(c.close),
                            if change >= 0. { "+" } else { "" }
                        ));
                        if c.volume > 0. {
                            tip.push_str(&format!("\nVolume {}", short_number(c.volume)));
                        }
                        tip
                    })
                    .collect();
                hit_columns(
                    key,
                    &vec![plot_w / n as f32; n],
                    plot_h,
                    tips,
                    &k.interact.on_select,
                )
            });
        let candles = k.candles.clone();
        let selected = k.interact.selected.filter(|i| *i < k.candles.len());
        let chart = framed(
            k.size[0],
            height,
            range,
            ticks,
            k.format,
            inline_labels,
            overlay,
            move |c, w, h, to_y| {
                let slot = w / n as f32;
                let body = (slot * 0.6).clamp(1., 18.);
                if let Some(i) = selected {
                    c.fill(
                        VectorPath::rectangle(i as f32 * slot, 0., slot, h),
                        ColorToken::BaseContent.alpha(9),
                    );
                }
                let body_rect = |i: usize, k: &Candle| {
                    let x = (i as f32 + 0.5) * slot;
                    let (top, bottom) = (to_y(k.open.max(k.close)), to_y(k.open.min(k.close)));
                    VectorPath::rectangle(x - body / 2., top, body, (bottom - top).max(1.))
                };
                // [rising, falling] wicks and bodies, each batched by colour.
                let mut wicks = [PathBatch::new(), PathBatch::new()];
                let mut bodies = [PathBatch::new(), PathBatch::new()];
                for (i, k) in candles.iter().enumerate() {
                    let side = usize::from(k.close < k.open);
                    let wick_x = ((i as f32 + 0.5) * slot).round() + 0.5;
                    wicks[side].add(
                        VectorPath::new()
                            .move_to(wick_x, to_y(k.high))
                            .line_to(wick_x, to_y(k.low)),
                    );
                    bodies[side].add(body_rect(i, k));
                }
                let [up_wicks, down_wicks] = wicks;
                let [up_bodies, down_bodies] = bodies;
                up_wicks.stroke(c, ColorToken::Success, 1.);
                down_wicks.stroke(c, ColorToken::Error, 1.);
                up_bodies.fill(c, ColorToken::Success);
                down_bodies.fill(c, ColorToken::Error);
                if let Some(i) = selected {
                    c.stroke(body_rect(i, &candles[i]), ColorToken::BaseContent, 1.5);
                    // The selected close as a dashed level across the plot.
                    let y = to_y(candles[i].close).round() + 0.5;
                    let mut dashes = PathBatch::new();
                    let mut x = 0.;
                    while x < w {
                        dashes.add(VectorPath::new().move_to(x, y).line_to((x + 4.).min(w), y));
                        x += 8.;
                    }
                    dashes.stroke(c, ColorToken::BaseContent.alpha(50), 1.);
                }
            },
        );
        if !k.volume {
            return chart;
        }
        let candles = k.candles;
        let max_v = candles
            .iter()
            .map(|c| c.volume)
            .fold(0f32, f32::max)
            .max(1.);
        let strip = canvas(move |c| {
            let slot = plot_w / n as f32;
            let body = (slot * 0.6).clamp(1., 18.);
            // [rising, falling] bars; the selected bar is drawn stronger.
            let mut bars = [PathBatch::new(), PathBatch::new()];
            let mut picked = None;
            for (i, k) in candles.iter().enumerate() {
                let h = k.volume / max_v * (vol_h - 4.);
                let x = (i as f32 + 0.5) * slot;
                let bar = VectorPath::rectangle(x - body / 2., vol_h - h, body, h.max(1.));
                let side = usize::from(k.close < k.open);
                if selected == Some(i) {
                    picked = Some((bar, side));
                } else {
                    bars[side].add(bar);
                }
            }
            let [up, down] = bars;
            up.fill(c, ColorToken::Success.alpha(45));
            down.fill(c, ColorToken::Error.alpha(45));
            if let Some((bar, side)) = picked {
                let token = [ColorToken::Success, ColorToken::Error][side];
                c.fill(bar, token.alpha(80));
            }
        })
        .width(plot_w)
        .height(vol_h);
        let mut parts = vec![chart, hstack![spacer().width(AXIS).flex(0.), strip].into()];
        if !trailing_labels.is_empty() {
            parts.push(x_label_row(trailing_labels, k.size[0]));
        }
        vstack(parts).gap(4., Axis::Vertical).into()
    }
}

// ── Donut ───────────────────────────────────────────────────────────────

/// Parts of a whole as a ring, with an optional centre label and legend.
pub struct DonutChart<C> {
    slices: Vec<(String, f32)>,
    size: f32,
    thickness: f32,
    center: Option<(String, String)>,
    legend: bool,
    format: Format,
    interact: Interact<C>,
}
pub fn donut_chart<C>(slices: impl IntoIterator<Item = (impl Into<String>, f32)>) -> DonutChart<C> {
    DonutChart {
        slices: slices
            .into_iter()
            .map(|(l, v)| (l.into(), v))
            .filter(|(_, v)| v.is_finite() && *v > 0.)
            .collect(),
        size: 180.,
        thickness: 26.,
        center: None,
        legend: true,
        format: short_number,
        interact: Interact::default(),
    }
}
interactive!(DonutChart);
impl<C> DonutChart<C> {
    pub fn size(mut self, diameter: f32) -> Self {
        self.size = diameter;
        self
    }
    pub fn thickness(mut self, value: f32) -> Self {
        self.thickness = value;
        self
    }
    /// A large value with a small caption in the hole, e.g. ("$48k", "Revenue").
    pub fn center(mut self, value: impl Into<String>, caption: impl Into<String>) -> Self {
        self.center = Some((value.into(), caption.into()));
        self
    }
    pub fn legend(mut self, value: bool) -> Self {
        self.legend = value;
        self
    }
    pub fn value_format(mut self, f: fn(f32) -> String) -> Self {
        self.format = f;
        self
    }
}
/// An annular sector from `a0` to `a1` radians (0 = 12 o'clock, clockwise).
fn sector(cx: f32, cy: f32, r: f32, inner: f32, a0: f32, a1: f32) -> VectorPath {
    let p = |rad: f32, a: f32| [cx + rad * a.sin(), cy - rad * a.cos()];
    let large = u8::from(a1 - a0 > std::f32::consts::PI);
    let (o0, o1, i1, i0) = (p(r, a0), p(r, a1), p(inner, a1), p(inner, a0));
    let d = format!(
        "M{} {} A{r} {r} 0 {large} 1 {} {} L{} {} A{inner} {inner} 0 {large} 0 {} {} Z",
        o0[0], o0[1], o1[0], o1[1], i1[0], i1[1], i0[0], i0[1]
    );
    VectorPath::from_svg(&d).unwrap_or_default()
}
/// Start and end angles of each slice (with a small gap between slices).
fn slice_angles(values: &[f32]) -> Vec<(f32, f32)> {
    let total: f32 = values.iter().sum();
    let gap = if values.len() > 1 { 0.025 } else { 0. };
    let mut a = 0.;
    values
        .iter()
        .map(|v| {
            let sweep = if total > 0. {
                v / total * std::f32::consts::TAU
            } else {
                0.
            };
            let span = (a + gap / 2., a + sweep - gap / 2.);
            a += sweep;
            span
        })
        .collect()
}
impl<C: 'static> From<DonutChart<C>> for Element<C> {
    fn from(d: DonutChart<C>) -> Self {
        let total: f32 = d.slices.iter().map(|(_, v)| v).sum();
        let (size, thick) = (d.size, d.thickness.min(d.size / 2.));
        let values: Vec<f32> = d.slices.iter().map(|(_, v)| *v).collect();
        let angles = slice_angles(&values);
        let selected = d.interact.selected.filter(|i| *i < values.len());
        // The selected slice is drawn at full radius; the others leave room.
        let pop = 5f32.min(size * 0.04);
        let ring_angles = angles.clone();
        let ring = canvas(move |c| {
            let (cx, cy, r) = (size / 2., size / 2., size / 2.);
            if total <= 0. {
                // A full turn's endpoints coincide (no arc); stop just short.
                c.fill(
                    sector(cx, cy, r, r - thick, 0., std::f32::consts::TAU - 1e-3),
                    ColorToken::Border,
                );
                return;
            }
            for (i, (a0, a1)) in ring_angles.iter().enumerate() {
                if a1 <= a0 {
                    continue;
                }
                let (outer, inner) = if selected == Some(i) {
                    (r, r - thick - pop)
                } else {
                    (r - pop, r - thick)
                };
                let color = series_color(i);
                let paint = match selected {
                    Some(s) if s != i => faded(color, 55),
                    _ => color.to_owned(),
                };
                c.fill(sector(cx, cy, outer, inner, *a0, *a1), paint);
            }
        })
        .width(size)
        .height(size);
        let mut layers: Vec<Element<C>> = vec![ring];
        let pct = |v: f32| if total > 0. { v / total * 100. } else { 0. };
        // Hit targets: a full-size button per slice, clipped to its sector.
        if let Some(key) = d.interact.key.as_deref() {
            for (i, ((label, v), (a0, a1))) in d.slices.iter().zip(&angles).enumerate() {
                if a1 <= a0 {
                    continue;
                }
                let tip = format!("{label}\n{} · {:.0}%", (d.format)(*v), pct(*v));
                let r = size / 2.;
                layers.push(
                    zstack![hit_target(
                        key,
                        i,
                        [size, size],
                        tip,
                        d.interact.on_select.clone()
                    )]
                    .key(format!("{key}-{i}-slice"))
                    .width(size)
                    .height(size)
                    .clip(Clip::Path(sector(r, r, r, r - thick - pop, *a0, *a1)))
                    .into(),
                );
            }
        }
        if let Some((value, caption)) = d.center {
            layers.push(
                vstack![
                    text(value)
                        .font_size((size * 0.13).max(14.))
                        .font_weight(700),
                    text(caption).font_size(11.).color(ColorToken::Muted),
                ]
                .width(size)
                .height(size)
                .flow(Flow::Center)
                .align(Align::Center)
                .gap(2., Axis::Vertical)
                .scroll_passthrough(true)
                .into(),
            );
        }
        // Never squeezed by a crowded row: hit sectors match the drawing.
        let donut: Element<C> = zstack(layers)
            .width(size)
            .height(size)
            .min_width(size)
            .min_height(size)
            .into();
        if !d.legend {
            return donut;
        }
        let rows: Vec<Element<C>> = d
            .slices
            .iter()
            .enumerate()
            .map(|(i, (label, v))| {
                let row: Element<C> = hstack![
                    vstack![]
                        .width(10.)
                        .height(10.)
                        .rounded(3.)
                        .background(series_color(i)),
                    text(label.clone())
                        .font_size(13.)
                        .font_weight(if selected == Some(i) { 600 } else { 400 })
                        .no_wrap(true)
                        .flex(1.),
                    text(format!("{:.0}%", pct(*v)))
                        .font_size(13.)
                        .font_weight(600)
                        .color(ColorToken::Muted),
                ]
                .gap(8., Axis::Horizontal)
                .align(Align::Center)
                .min_width(110.)
                .height(26.)
                .padding(4., Edge::Horizontal)
                .scroll_passthrough(true)
                .into();
                // Legend rows select their slice too.
                match (d.interact.key.as_deref(), d.interact.on_select.clone()) {
                    (Some(key), Some(f)) => zstack![
                        button("")
                            .key(format!("{key}-legend-{i}"))
                            .accessible_label(format!("{label}, {:.0}%", pct(*v)))
                            .variant(ButtonVariant::Ghost)
                            .height(26.)
                            .rounded(6.)
                            .on_click(move |app: &mut C| (f.borrow_mut())(app, i)),
                        row,
                    ]
                    .key(format!("{key}-legend-{i}-row"))
                    .height(26.)
                    .align(Align::Stretch)
                    .into(),
                    _ => row,
                }
            })
            .collect();
        hstack![donut, vstack(rows).gap(2., Axis::Vertical).flex(1.)]
            .gap(16., Axis::Horizontal)
            .align(Align::Center)
            .into()
    }
}

// ── Stacked bars ────────────────────────────────────────────────────────

/// Bars per category with series stacked on top of each other.
pub struct StackedBarChart<C> {
    categories: Vec<String>,
    series: Vec<(String, Vec<f32>)>,
    size: [f32; 2],
    legend: bool,
    format: Format,
    interact: Interact<C>,
}
pub fn stacked_bar_chart<C>(
    categories: impl IntoIterator<Item = impl Into<String>>,
    series: impl IntoIterator<Item = (impl Into<String>, Vec<f32>)>,
) -> StackedBarChart<C> {
    StackedBarChart {
        categories: categories.into_iter().map(Into::into).collect(),
        series: series.into_iter().map(|(n, v)| (n.into(), v)).collect(),
        size: [480., 240.],
        legend: true,
        format: short_number,
        interact: Interact::default(),
    }
}
interactive!(StackedBarChart);
impl<C> StackedBarChart<C> {
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.size = [width, height];
        self
    }
    pub fn legend(mut self, value: bool) -> Self {
        self.legend = value;
        self
    }
    pub fn value_format(mut self, f: fn(f32) -> String) -> Self {
        self.format = f;
        self
    }
}
impl<C: 'static> From<StackedBarChart<C>> for Element<C> {
    fn from(b: StackedBarChart<C>) -> Self {
        let n = b.categories.len();
        let value = |s: usize, i: usize| {
            b.series[s]
                .1
                .get(i)
                .copied()
                .filter(|v| v.is_finite() && *v > 0.)
                .unwrap_or(0.)
        };
        let totals: Vec<f32> = (0..n)
            .map(|i| (0..b.series.len()).map(|s| value(s, i)).sum())
            .collect();
        let hi = totals.iter().copied().fold(0f32, f32::max);
        let (ticks, range) = nice_ticks(0., hi.max(1.), 4);
        let height = b.size[1] - if b.legend { 26. } else { 0. };
        let plot_w = b.size[0] - AXIS;
        let plot_h = (height - X_LABELS).max(10.);
        let x_labels = spread_labels(&b.categories, plot_w, true);
        let names: Vec<String> = b.series.iter().map(|(n, _)| n.clone()).collect();
        let overlay = b.interact.key.as_deref().filter(|_| n > 0).map(|key| {
            let tips = (0..n)
                .map(|i| {
                    let mut tip = b.categories[i].clone();
                    for (s, name) in names.iter().enumerate() {
                        tip.push_str(&format!("\n{name}: {}", (b.format)(value(s, i))));
                    }
                    tip.push_str(&format!("\nTotal: {}", (b.format)(totals[i])));
                    tip
                })
                .collect();
            hit_columns(
                key,
                &vec![plot_w / n as f32; n],
                plot_h,
                tips,
                &b.interact.on_select,
            )
        });
        let series: Vec<Vec<f32>> = (0..b.series.len())
            .map(|s| (0..n).map(|i| value(s, i)).collect())
            .collect();
        let selected = b.interact.selected.filter(|i| *i < n);
        let chart = framed(
            b.size[0],
            height,
            range,
            ticks,
            b.format,
            x_labels,
            overlay,
            move |c, w, _h, to_y| {
                let slot = w / n.max(1) as f32;
                let bar = (slot * 0.6).min(40.);
                // Per series: [normal, faded]. Other columns step back while
                // one is selected.
                let mut batches: Vec<[PathBatch; 2]> = series
                    .iter()
                    .map(|_| [PathBatch::new(), PathBatch::new()])
                    .collect();
                for i in 0..n {
                    let x = (i as f32 + 0.5) * slot - bar / 2.;
                    let faded_column = usize::from(selected.is_some_and(|sel| sel != i));
                    let mut base = 0.;
                    for (s, values) in series.iter().enumerate() {
                        let v = values[i];
                        if v <= 0. {
                            continue;
                        }
                        let (top, bottom) = (to_y(base + v), to_y(base));
                        batches[s][faded_column].add(VectorPath::rectangle(
                            x,
                            top,
                            bar,
                            (bottom - top - 1.).max(1.),
                        ));
                        base += v;
                    }
                }
                for (s, [normal, dim]) in batches.into_iter().enumerate() {
                    let color = series_color(s);
                    normal.fill(c, color);
                    dim.fill(c, faded(color, 40));
                }
            },
        );
        if b.legend {
            vstack![chart, legend(&names)]
                .gap(10., Axis::Vertical)
                .into()
        } else {
            chart
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nice_ticks_are_round_and_cover_the_range() {
        let (t, r) = nice_ticks(3., 97., 4);
        assert_eq!(t, [0., 25., 50., 75., 100.]);
        assert_eq!(r, (0., 100.));
        let (t, _) = nice_ticks(0.12, 0.81, 4);
        assert!(t.first().unwrap() <= &0.12 && t.last().unwrap() >= &0.81);
        assert!(t.windows(2).all(|w| ((w[1] - w[0]) - 0.2).abs() < 1e-4));
        let (t, _) = nice_ticks(5., 5., 4);
        assert!(t.len() >= 2, "flat data still gets an axis");
    }
    #[test]
    fn short_numbers_read_well() {
        assert_eq!(short_number(0.), "0");
        assert_eq!(short_number(950.), "950");
        assert_eq!(short_number(1500.), "1.5k");
        assert_eq!(short_number(2_000_000.), "2M");
        assert_eq!(short_number(0.25), "0.25");
        assert_eq!(short_number(-12_500.), "-12.5k");
    }
    #[test]
    fn charts_build_and_reject_bad_data_quietly() {
        let e: Element<()> = vstack![
            area_chart([("A", vec![1., 3., 2.]), ("B", vec![2., 1., 4.])])
                .stacked(true)
                .x_labels(["Mon", "Tue", "Wed"]),
            candlestick_chart([
                Candle::new(10., 12., 9., 11.).volume(100.),
                Candle::new(11., 11.5, 8., 9.).volume(80.),
                Candle::new(f32::NAN, 1., 0., 1.),
            ])
            .volume(true),
            donut_chart([("Web", 60.), ("App", 30.), ("Zero", 0.)]).center("90", "Visits"),
            stacked_bar_chart(
                ["Q1", "Q2"],
                [("North", vec![3., 4.]), ("South", vec![2., -1.])]
            ),
            donut_chart(Vec::<(&str, f32)>::new()),
        ]
        .into();
        e.build().unwrap();
        let sector = sector(50., 50., 50., 30., 0., 1.5);
        assert!(sector.is_valid());
    }
}

// ── Draw-in, axes and readouts shared by the plots below ────────────────

/// A draw-in: 28 frames 24 ms apart (about 0.7 s), progress eased out.
const DRAW_FRAMES: usize = 28;
const DRAW_FRAME_MS: u32 = 24;
fn draw_in_steps() -> impl Iterator<Item = f32> {
    (1..=DRAW_FRAMES).map(|i| {
        let t = i as f32 / DRAW_FRAMES as f32;
        1. - (1. - t).powi(3)
    })
}
/// A plot's canvas: horizontal gridlines at `grid` (y pixels), then `draw` at
/// progress `t`. With `animate`, the draw-in plays once per revision
/// (`canvas_frames`), and reduced motion shows the last frame at once.
fn plot_canvas<C>(
    size: [f32; 2],
    grid: Vec<f32>,
    animate: Option<u64>,
    key: Option<&str>,
    draw: impl Fn(&mut Canvas, f32) + 'static,
) -> Element<C> {
    let [w, h] = size;
    let paint = move |c: &mut Canvas, t: f32| {
        for y in &grid {
            let y = y.round() + 0.5;
            c.stroke(
                VectorPath::new().move_to(0., y).line_to(w, y),
                ColorToken::Border,
                1.,
            );
        }
        draw(c, t);
    };
    let element: Element<C> = match animate {
        Some(revision) => {
            let frames = draw_in_steps()
                .map(|t| {
                    let mut c = Canvas::default();
                    paint(&mut c, t);
                    c
                })
                .collect();
            let e = canvas_frames(frames, DRAW_FRAME_MS, revision);
            match key {
                Some(k) => e.key(format!("{k}-plot")),
                None => e,
            }
        }
        None => canvas(move |c| paint(c, 1.)),
    };
    element.width(w).height(h)
}
/// Labels at the left of a plot, each centred on its y.
fn y_axis<C>(labels: Vec<(f32, String)>, plot_h: f32) -> Element<C> {
    let labels: Vec<Element<C>> = labels
        .into_iter()
        .map(|(y, label)| {
            hstack![text(label)
                .font_size(11.)
                .color(ColorToken::Muted)
                .no_wrap(true)]
            .width(AXIS - 8.)
            .justify(Justify::End)
            .padding((y - 7.).clamp(0., (plot_h - 14.).max(0.)), Edge::Top)
            .into()
        })
        .collect();
    zstack(labels)
        .width(AXIS)
        .height(plot_h)
        .flow(Flow::TopLeft)
        .into()
}
/// The selected point's description above the plot, for touch screens where
/// tooltips don't show; a hint when nothing is selected.
fn readout<C>(line: Option<String>) -> Element<C> {
    let t = match line {
        Some(line) => text(line.replace('\n', " · "))
            .font_size(12.)
            .font_weight(600)
            .color(ColorToken::BaseContent),
        None => text("Tap the chart to read a value")
            .font_size(12.)
            .color(ColorToken::Muted),
    };
    hstack![t.no_wrap(true)]
        .height(18.)
        .padding(AXIS, Edge::Left)
        .into()
}
/// Plot, axes, labels and optional readout and legend, stacked.
#[allow(clippy::too_many_arguments)]
fn assemble<C>(
    width: f32,
    plot: Element<C>,
    plot_size: [f32; 2],
    overlays: Vec<Element<C>>,
    y_labels: Vec<(f32, String)>,
    x_labels: Vec<(f32, String)>,
    readout_line: Option<Option<String>>,
    legend_names: Option<&[String]>,
) -> Element<C> {
    let [plot_w, plot_h] = plot_size;
    let mut layers = vec![plot];
    layers.extend(overlays);
    let plot: Element<C> = zstack(layers)
        .width(plot_w)
        .height(plot_h)
        .flow(Flow::TopLeft)
        .into();
    let mut parts = Vec::new();
    if let Some(line) = readout_line {
        parts.push(readout(line));
    }
    parts.push(
        hstack![y_axis(y_labels, plot_h), plot]
            .align(Align::Start)
            .into(),
    );
    if !x_labels.is_empty() {
        parts.push(x_label_row(x_labels, width));
    }
    if let Some(names) = legend_names {
        parts.push(legend(names));
    }
    vstack(parts).gap(6., Axis::Vertical).width(width).into()
}
/// Tick labels for `range` and the y mapping, inside a plot `plot_h` tall.
fn value_axis(range: (f32, f32), plot_h: f32) -> impl Fn(f32) -> f32 + Copy {
    move |v: f32| plot_h - (v - range.0) / (range.1 - range.0).max(f32::EPSILON) * plot_h
}
/// A polyline through `points`, cut at `x_max` (interpolating the last point).
fn cut_polyline(points: &[[f32; 2]], x_max: f32) -> Vec<[f32; 2]> {
    let mut out = Vec::new();
    for (i, p) in points.iter().enumerate() {
        if p[0] <= x_max {
            out.push(*p);
        } else {
            if let Some(prev) = i.checked_sub(1).map(|j| points[j]) {
                let t = (x_max - prev[0]) / (p[0] - prev[0]).max(f32::EPSILON);
                out.push([x_max, prev[1] + (p[1] - prev[1]) * t]);
            }
            break;
        }
    }
    out
}
/// A run split into pieces of at most 400 points that share their ends, so
/// no path passes the 1,024-command limit however long the data.
fn pieces(run: &[[f32; 2]]) -> Vec<&[[f32; 2]]> {
    const MOST: usize = 400;
    let mut out = Vec::new();
    let mut start = 0;
    while start + 1 < run.len() {
        let end = (start + MOST).min(run.len());
        out.push(&run[start..end]);
        start = end - 1;
    }
    out
}
fn through(points: &[[f32; 2]]) -> VectorPath {
    let mut path = VectorPath::new().move_to(points[0][0], points[0][1]);
    for p in &points[1..] {
        path = path.line_to(p[0], p[1]);
    }
    path
}

// ── Line plot ───────────────────────────────────────────────────────────

/// Lines over shared x positions, built for real-world readings: a missing
/// value (`f32::NAN`) leaves a gap instead of a line to zero; dashed guides
/// mark reference values; lines can glow, sit over a soft area and draw
/// themselves in. With `.key(..)`, tapping (or hovering) reads a point, and
/// `.readout(true)` writes it above the plot for touch screens.
///
/// ```
/// use volt_ui_kit::ui::prelude::*;
/// let resting = vec![58., 57., f32::NAN, 60., 59.];
/// let _plot: Element<()> = line_plot([("Resting", resting)])
///     .x_labels(["Mon", "Tue", "Wed", "Thu", "Fri"])
///     .guide(60., "60")
///     .area(true)
///     .glow(true)
///     .animate(1)
///     .key("resting")
///     .readout(true)
///     .into();
/// ```
pub struct LinePlot<C> {
    series: Vec<(String, Vec<f32>)>,
    size: [f32; 2],
    x_labels: Vec<String>,
    range: Option<(f32, f32)>,
    guides: Vec<(f32, String)>,
    colors: Vec<String>,
    legend: bool,
    area: bool,
    glow: bool,
    points: bool,
    readout: bool,
    animate: Option<u64>,
    format: Format,
    interact: Interact<C>,
}
pub fn line_plot<C>(
    series: impl IntoIterator<Item = (impl Into<String>, Vec<f32>)>,
) -> LinePlot<C> {
    LinePlot {
        series: series.into_iter().map(|(n, v)| (n.into(), v)).collect(),
        size: [480., 220.],
        x_labels: Vec::new(),
        range: None,
        guides: Vec::new(),
        colors: Vec::new(),
        legend: true,
        area: false,
        glow: false,
        points: true,
        readout: false,
        animate: None,
        format: short_number,
        interact: Interact::default(),
    }
}
interactive!(LinePlot);
impl<C> LinePlot<C> {
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.size = [width, height];
        self
    }
    pub fn x_labels(mut self, labels: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.x_labels = labels.into_iter().map(Into::into).collect();
        self
    }
    /// A fixed value range instead of one fitted to the data.
    pub fn range(mut self, low: f32, high: f32) -> Self {
        self.range = Some((low, high));
        self
    }
    /// A dashed reference line at `value`, labelled at its right end.
    pub fn guide(mut self, value: f32, label: impl Into<String>) -> Self {
        self.guides.push((value, label.into()));
        self
    }
    /// Series colours (hex or theme tokens) instead of the default palette.
    pub fn colors(mut self, colors: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.colors = colors.into_iter().map(Into::into).collect();
        self
    }
    pub fn legend(mut self, value: bool) -> Self {
        self.legend = value;
        self
    }
    /// A soft fill under each line, fading to transparent.
    pub fn area(mut self, value: bool) -> Self {
        self.area = value;
        self
    }
    /// A glow under each line.
    pub fn glow(mut self, value: bool) -> Self {
        self.glow = value;
        self
    }
    /// Dots on each value while they stay apart (default on).
    pub fn points(mut self, value: bool) -> Self {
        self.points = value;
        self
    }
    /// Write the selected point above the plot (for touch screens).
    pub fn readout(mut self, value: bool) -> Self {
        self.readout = value;
        self
    }
    /// Draw the lines in from the left, once per `revision`: pass a number
    /// that changes when the data does.
    pub fn animate(mut self, revision: u64) -> Self {
        self.animate = Some(revision);
        self
    }
    pub fn value_format(mut self, f: fn(f32) -> String) -> Self {
        self.format = f;
        self
    }
}
impl<C: 'static> From<LinePlot<C>> for Element<C> {
    fn from(a: LinePlot<C>) -> Self {
        let n = a.series.iter().map(|(_, v)| v.len()).max().unwrap_or(0);
        let finite: Vec<f32> = a
            .series
            .iter()
            .flat_map(|(_, v)| v.iter().copied())
            .chain(a.guides.iter().map(|(v, _)| *v))
            .filter(|v| v.is_finite())
            .collect();
        let (ticks, range) = match a.range.filter(|(lo, hi)| lo.is_finite() && hi > lo) {
            Some((lo, hi)) => {
                let (ticks, _) = nice_ticks(lo, hi, 4);
                (
                    ticks.into_iter().filter(|t| *t >= lo && *t <= hi).collect(),
                    (lo, hi),
                )
            }
            None => {
                let lo = finite.iter().copied().fold(f32::MAX, f32::min);
                let hi = finite.iter().copied().fold(f32::MIN, f32::max);
                if finite.is_empty() {
                    nice_ticks(0., 1., 4)
                } else {
                    nice_ticks(lo, hi.max(lo + 1.), 4)
                }
            }
        };
        let names: Vec<String> = a.series.iter().map(|(n, _)| n.clone()).collect();
        let show_legend = a.legend && a.series.len() > 1;
        let interactive = a.interact.key.is_some();
        let width = a.size[0];
        let plot_w = (width - AXIS).max(10.);
        let plot_h = (a.size[1]
            - X_LABELS
            - if show_legend { 26. } else { 0. }
            - if a.readout && interactive { 24. } else { 0. })
        .max(10.);
        let to_y = value_axis(range, plot_h);
        let x_at = move |i: usize| {
            if n > 1 {
                i as f32 * plot_w / (n - 1) as f32
            } else {
                plot_w / 2.
            }
        };
        let colors: Vec<String> = (0..a.series.len())
            .map(|i| {
                a.colors
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| series_color(i).to_owned())
            })
            .collect();
        // Runs of present values per series, in plot pixels.
        let runs: Vec<Vec<Vec<[f32; 2]>>> = a
            .series
            .iter()
            .map(|(_, v)| {
                let mut runs = vec![Vec::new()];
                for (i, value) in v.iter().enumerate() {
                    if value.is_finite() {
                        runs.last_mut().unwrap().push([x_at(i), to_y(*value)]);
                    } else if !runs.last().unwrap().is_empty() {
                        runs.push(Vec::new());
                    }
                }
                runs.retain(|r| !r.is_empty());
                runs
            })
            .collect();
        let value = |s: usize, x: usize| a.series[s].1.get(x).copied().filter(|v| v.is_finite());
        let tip = |x: usize| {
            let mut tip = a
                .x_labels
                .get(x)
                .cloned()
                .unwrap_or_else(|| format!("#{}", x + 1));
            for (s, name) in names.iter().enumerate() {
                let v = value(s, x).map_or("no value".to_owned(), a.format);
                tip.push_str(&format!("\n{name}: {v}"));
            }
            tip
        };
        let selected = a.interact.selected.filter(|i| *i < n);
        let readout_line = (a.readout && interactive).then(|| selected.map(tip));
        let overlay = a.interact.key.as_deref().filter(|_| n > 1).map(|key| {
            let dx = plot_w / (n - 1) as f32;
            let widths: Vec<f32> = (0..n)
                .map(|i| if i == 0 || i == n - 1 { dx / 2. } else { dx })
                .collect();
            hit_columns(
                key,
                &widths,
                plot_h,
                (0..n).map(tip).collect(),
                &a.interact.on_select,
            )
        });
        let selected_points: Vec<(f32, Vec<(f32, String)>)> = selected
            .map(|x| {
                let ys = (0..a.series.len())
                    .filter_map(|s| value(s, x).map(|v| (to_y(v), colors[s].clone())))
                    .collect();
                vec![(x_at(x), ys)]
            })
            .unwrap_or_default();
        let guides: Vec<f32> = a.guides.iter().map(|(v, _)| to_y(*v)).collect();
        let (area, glow, dots) = (
            a.area,
            a.glow,
            a.points && n > 1 && plot_w / (n - 1).max(1) as f32 >= 6.,
        );
        let draw_colors = colors.clone();
        let plot = plot_canvas(
            [plot_w, plot_h],
            ticks.iter().map(|t| to_y(*t)).collect(),
            a.animate,
            a.interact.key.as_deref(),
            move |c, t| {
                let reach = plot_w * t;
                for y in &guides {
                    c.stroke_with(
                        VectorPath::new().move_to(0., *y).line_to(plot_w, *y),
                        ColorToken::BaseContent.alpha(40),
                        StrokeStyle::new(1.).dash([4., 4.]),
                    );
                }
                // Later series first, so the first stays on top. Each series
                // is one stroke and one area, its runs as subpaths, so gaps
                // never multiply the canvas's paths.
                for (s, series_runs) in runs.iter().enumerate().rev() {
                    let color = &draw_colors[s];
                    let (mut lines, mut fills) = (PathBatch::new(), PathBatch::new());
                    let mut marks = PathBatch::new();
                    for run in series_runs {
                        let shown = if t >= 1. {
                            run.clone()
                        } else {
                            cut_polyline(run, reach)
                        };
                        for piece in pieces(&shown) {
                            lines.add(through(piece));
                            let (first, last) = (piece[0], piece[piece.len() - 1]);
                            fills.add(
                                through(piece)
                                    .line_to(last[0], plot_h)
                                    .line_to(first[0], plot_h)
                                    .close(),
                            );
                        }
                        // A lone value between gaps still shows, as a dot.
                        let lone = run.len() == 1 && shown.len() == 1;
                        for p in shown.iter().filter(|p| (dots || lone) && run.contains(p)) {
                            marks.add(VectorPath::circle(p[0], p[1], 2.5));
                        }
                    }
                    if area {
                        for fill in fills.0 {
                            c.fill_gradient(
                                fill,
                                Gradient::linear(
                                    180.,
                                    [(0., faded(color, 28)), (1., faded(color, 0))],
                                ),
                            );
                        }
                    }
                    for line in lines.0 {
                        let style = StrokeStyle::new(2.).round();
                        if glow {
                            c.glow_stroke(line, color, style, 1.);
                        } else {
                            c.stroke_with(line, color.clone(), style);
                        }
                    }
                    marks.fill(c, color.clone());
                }
                if t >= 1. {
                    for (x, ys) in &selected_points {
                        let px = x.round() + 0.5;
                        c.stroke(
                            VectorPath::new().move_to(px, 0.).line_to(px, plot_h),
                            ColorToken::BaseContent.alpha(45),
                            1.,
                        );
                        for (y, color) in ys {
                            c.fill(VectorPath::circle(px, *y, 5.5), ColorToken::Base100);
                            c.fill(VectorPath::circle(px, *y, 3.5), color.clone());
                        }
                    }
                }
            },
        );
        // Guide labels at the right end of their lines.
        let guide_labels: Vec<Element<C>> = a
            .guides
            .iter()
            .map(|(v, label)| {
                hstack![text(label.clone())
                    .font_size(10.)
                    .color(ColorToken::Muted)
                    .no_wrap(true)]
                .width(plot_w)
                .justify(Justify::End)
                .padding(
                    (to_y(*v) - 14.).clamp(0., (plot_h - 12.).max(0.)),
                    Edge::Top,
                )
                .into()
            })
            .collect();
        let mut overlays = Vec::new();
        if !guide_labels.is_empty() {
            overlays.push(
                zstack(guide_labels)
                    .width(plot_w)
                    .height(plot_h)
                    .flow(Flow::TopLeft)
                    .into(),
            );
        }
        overlays.extend(overlay);
        assemble(
            width,
            plot,
            [plot_w, plot_h],
            overlays,
            ticks.iter().map(|t| (to_y(*t), (a.format)(*t))).collect(),
            spread_labels(&a.x_labels, plot_w, false),
            readout_line,
            show_legend.then_some(names.as_slice()),
        )
    }
}

// ── Range chart ─────────────────────────────────────────────────────────

/// One column of a range chart: lowest and highest, and a middle value
/// (median, mean or close) shown as a dot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ValueRange {
    pub low: f32,
    pub high: f32,
    pub mid: Option<f32>,
}
impl ValueRange {
    pub fn new(low: f32, high: f32) -> Self {
        Self {
            low: low.min(high),
            high: high.max(low),
            mid: None,
        }
    }
    pub fn mid(mut self, value: f32) -> Self {
        self.mid = Some(value);
        self
    }
    fn present(&self) -> bool {
        self.low.is_finite() && self.high.is_finite()
    }
}
/// Columns from lowest to highest with a dot for the middle: daily ranges of
/// heart rate, temperature or prices without open/close. Ranges grow from
/// their middle when `.animate(..)` is set.
pub struct RangeChart<C> {
    columns: Vec<ValueRange>,
    size: [f32; 2],
    x_labels: Vec<String>,
    range: Option<(f32, f32)>,
    color: String,
    readout: bool,
    animate: Option<u64>,
    format: Format,
    interact: Interact<C>,
}
pub fn range_chart<C>(columns: impl IntoIterator<Item = ValueRange>) -> RangeChart<C> {
    RangeChart {
        columns: columns.into_iter().collect(),
        size: [480., 220.],
        x_labels: Vec::new(),
        range: None,
        color: "@primary".into(),
        readout: false,
        animate: None,
        format: short_number,
        interact: Interact::default(),
    }
}
interactive!(RangeChart);
impl<C> RangeChart<C> {
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.size = [width, height];
        self
    }
    pub fn x_labels(mut self, labels: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.x_labels = labels.into_iter().map(Into::into).collect();
        self
    }
    pub fn range(mut self, low: f32, high: f32) -> Self {
        self.range = Some((low, high));
        self
    }
    pub fn color(mut self, color: impl Into<String>) -> Self {
        self.color = color.into();
        self
    }
    pub fn readout(mut self, value: bool) -> Self {
        self.readout = value;
        self
    }
    pub fn animate(mut self, revision: u64) -> Self {
        self.animate = Some(revision);
        self
    }
    pub fn value_format(mut self, f: fn(f32) -> String) -> Self {
        self.format = f;
        self
    }
}
impl<C: 'static> From<RangeChart<C>> for Element<C> {
    fn from(a: RangeChart<C>) -> Self {
        let n = a.columns.len();
        let present: Vec<&ValueRange> = a.columns.iter().filter(|c| c.present()).collect();
        let (ticks, range) = match a.range.filter(|(lo, hi)| lo.is_finite() && hi > lo) {
            Some((lo, hi)) => (
                nice_ticks(lo, hi, 4)
                    .0
                    .into_iter()
                    .filter(|t| *t >= lo && *t <= hi)
                    .collect(),
                (lo, hi),
            ),
            None if present.is_empty() => nice_ticks(0., 1., 4),
            None => {
                let lo = present.iter().map(|c| c.low).fold(f32::MAX, f32::min);
                let hi = present.iter().map(|c| c.high).fold(f32::MIN, f32::max);
                nice_ticks(lo, hi.max(lo + 1.), 4)
            }
        };
        let interactive = a.interact.key.is_some();
        let width = a.size[0];
        let plot_w = (width - AXIS).max(10.);
        let plot_h =
            (a.size[1] - X_LABELS - if a.readout && interactive { 24. } else { 0. }).max(10.);
        let to_y = value_axis(range, plot_h);
        let slot = plot_w / n.max(1) as f32;
        let bar = (slot * 0.42).clamp(3., 18.);
        let format = a.format;
        let tip = |i: usize| {
            let c = a.columns[i];
            let label = a
                .x_labels
                .get(i)
                .cloned()
                .unwrap_or_else(|| format!("#{}", i + 1));
            if !c.present() {
                return format!("{label}\nno value");
            }
            let mut tip = format!("{label}\n{} to {}", format(c.low), format(c.high));
            if let Some(m) = c.mid.filter(|m| m.is_finite()) {
                tip.push_str(&format!(", middle {}", format(m)));
            }
            tip
        };
        let selected = a.interact.selected.filter(|i| *i < n);
        let readout_line = (a.readout && interactive).then(|| selected.map(tip));
        let overlay = a.interact.key.as_deref().filter(|_| n > 0).map(|key| {
            hit_columns(
                key,
                &vec![slot; n],
                plot_h,
                (0..n).map(tip).collect(),
                &a.interact.on_select,
            )
        });
        let columns = a.columns.clone();
        let color = a.color.clone();
        let plot = plot_canvas(
            [plot_w, plot_h],
            ticks.iter().map(|t| to_y(*t)).collect(),
            a.animate,
            a.interact.key.as_deref(),
            move |c, t| {
                if let Some(i) = selected {
                    c.fill(
                        VectorPath::rounded_rectangle(i as f32 * slot, 0., slot, plot_h, 6.),
                        ColorToken::BaseContent.alpha(7),
                    );
                }
                // Capsules from low to high, rings and dots for the middles:
                // one batch each (columns never overlap), the selected capsule
                // apart, stronger.
                let (mut bars, mut rings, mut dots) =
                    (PathBatch::new(), PathBatch::new(), PathBatch::new());
                let mut chosen = None;
                for (i, col) in columns.iter().enumerate().filter(|(_, c)| c.present()) {
                    let x = (i as f32 + 0.5) * slot;
                    let mid = col
                        .mid
                        .filter(|m| m.is_finite())
                        .unwrap_or((col.low + col.high) / 2.);
                    let (top, bottom) = (
                        to_y(mid + (col.high - mid) * t),
                        to_y(mid + (col.low - mid) * t),
                    );
                    let capsule = VectorPath::rounded_rectangle(
                        x - bar / 2.,
                        top - 0.5,
                        bar,
                        (bottom - top).max(0.) + 1.,
                        bar / 2.,
                    );
                    if selected == Some(i) {
                        chosen = Some(capsule);
                    } else {
                        bars.add(capsule);
                    }
                    if let Some(m) = col.mid.filter(|m| m.is_finite()) {
                        let r = (bar * 0.55).max(3.) * t;
                        if r > 0. {
                            rings.add(VectorPath::circle(x, to_y(m), r + 1.5));
                            dots.add(VectorPath::circle(x, to_y(m), r));
                        }
                    }
                }
                bars.fill(c, faded(&color, 45));
                if let Some(capsule) = chosen {
                    c.fill(capsule, faded(&color, 80));
                }
                rings.fill(c, ColorToken::Base100);
                dots.fill(c, color.clone());
            },
        );
        assemble(
            width,
            plot,
            [plot_w, plot_h],
            overlay.into_iter().collect(),
            ticks.iter().map(|t| (to_y(*t), format(*t))).collect(),
            spread_labels(&a.x_labels, plot_w, true),
            readout_line,
            None,
        )
    }
}

// ── Timeline ────────────────────────────────────────────────────────────

/// One stretch of a state on a timeline: its lane and when it starts and
/// ends, in the chart's own units (minutes, hours, seconds).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimelineSpan {
    pub lane: usize,
    pub start: f32,
    pub end: f32,
}
/// States over time, one lane per state: sleep stages, a device's modes, a
/// build pipeline's steps. Spans that follow each other are joined by thin
/// connectors. It reveals from the left with `.animate(..)`.
pub struct TimelineChart<C> {
    lanes: Vec<(String, String)>,
    spans: Vec<TimelineSpan>,
    size: [f32; 2],
    domain: Option<(f32, f32)>,
    x_labels: Vec<String>,
    connectors: bool,
    readout: bool,
    animate: Option<u64>,
    format: Format,
    interact: Interact<C>,
}
/// `lanes` are (name, colour) from the top; `spans` are (lane, start, end).
pub fn timeline_chart<C>(
    lanes: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    spans: impl IntoIterator<Item = (usize, f32, f32)>,
) -> TimelineChart<C> {
    TimelineChart {
        lanes: lanes
            .into_iter()
            .map(|(n, c)| (n.into(), c.into()))
            .collect(),
        spans: spans
            .into_iter()
            .map(|(lane, start, end)| TimelineSpan {
                lane,
                start: start.min(end),
                end: end.max(start),
            })
            .collect(),
        size: [480., 180.],
        domain: None,
        x_labels: Vec::new(),
        connectors: true,
        readout: false,
        animate: None,
        format: short_number,
        interact: Interact::default(),
    }
}
interactive!(TimelineChart);
impl<C> TimelineChart<C> {
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.size = [width, height];
        self
    }
    /// The time span shown; defaults to the first start and last end.
    pub fn domain(mut self, start: f32, end: f32) -> Self {
        self.domain = Some((start, end));
        self
    }
    /// Labels spread evenly from the domain's start to its end.
    pub fn x_labels(mut self, labels: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.x_labels = labels.into_iter().map(Into::into).collect();
        self
    }
    /// Thin lines joining spans that follow each other (default on).
    pub fn connectors(mut self, value: bool) -> Self {
        self.connectors = value;
        self
    }
    pub fn readout(mut self, value: bool) -> Self {
        self.readout = value;
        self
    }
    pub fn animate(mut self, revision: u64) -> Self {
        self.animate = Some(revision);
        self
    }
    /// How a time in the chart's units reads in tooltips ("23:40", "2.5 h").
    pub fn value_format(mut self, f: fn(f32) -> String) -> Self {
        self.format = f;
        self
    }
}
impl<C: 'static> From<TimelineChart<C>> for Element<C> {
    fn from(a: TimelineChart<C>) -> Self {
        let lanes = a.lanes.len().max(1);
        let spans: Vec<TimelineSpan> = a
            .spans
            .iter()
            .copied()
            .filter(|s| s.lane < a.lanes.len() && s.start.is_finite() && s.end.is_finite())
            .collect();
        let (d0, d1) = a
            .domain
            .filter(|(s, e)| s.is_finite() && e > s)
            .unwrap_or_else(|| {
                let s = spans.iter().map(|s| s.start).fold(f32::MAX, f32::min);
                let e = spans.iter().map(|s| s.end).fold(f32::MIN, f32::max);
                if spans.is_empty() || e <= s {
                    (0., 1.)
                } else {
                    (s, e)
                }
            });
        let interactive = a.interact.key.is_some();
        let width = a.size[0];
        let plot_w = (width - AXIS).max(10.);
        let plot_h =
            (a.size[1] - X_LABELS - if a.readout && interactive { 24. } else { 0. }).max(10.);
        let lane_h = plot_h / lanes as f32;
        let x_of = move |v: f32| (v - d0) / (d1 - d0) * plot_w;
        let format = a.format;
        let names: Vec<String> = a.lanes.iter().map(|(n, _)| n.clone()).collect();
        let tip = |i: usize| {
            let s = spans[i];
            format!(
                "{}\n{} to {}",
                names[s.lane],
                format(s.start),
                format(s.end)
            )
        };
        let selected = a.interact.selected.filter(|i| *i < spans.len());
        let readout_line = (a.readout && interactive).then(|| selected.map(tip));
        // Hit targets sit on their spans, in their lanes.
        let overlay = a.interact.key.as_deref().map(|key| {
            let targets: Vec<Element<C>> = spans
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    let (x0, x1) = (
                        x_of(s.start).clamp(0., plot_w),
                        x_of(s.end).clamp(0., plot_w),
                    );
                    vstack![hit_target(
                        key,
                        i,
                        [(x1 - x0).max(4.), lane_h],
                        tip(i),
                        a.interact.on_select.clone(),
                    )]
                    .padding(x0, Edge::Left)
                    .padding(s.lane as f32 * lane_h, Edge::Top)
                    .into()
                })
                .collect();
            zstack(targets)
                .key(format!("{key}-hits"))
                .width(plot_w)
                .height(plot_h)
                .flow(Flow::TopLeft)
                .into()
        });
        let colors: Vec<String> = a.lanes.iter().map(|(_, c)| c.clone()).collect();
        let connectors = a.connectors;
        let draw_spans = spans.clone();
        let plot = plot_canvas(
            [plot_w, plot_h],
            (1..lanes).map(|i| i as f32 * lane_h).collect(),
            a.animate,
            a.interact.key.as_deref(),
            move |c, t| {
                let reach = plot_w * t;
                let bar = (lane_h * 0.62).max(2.);
                let centre = |lane: usize| (lane as f32 + 0.5) * lane_h;
                if connectors {
                    let mut joins = PathBatch::new();
                    for pair in draw_spans.windows(2) {
                        let (a, b) = (pair[0], pair[1]);
                        let x = x_of(a.end).round() + 0.5;
                        if a.lane != b.lane
                            && (b.start - a.end).abs() <= (d1 - d0) * 1e-3
                            && x <= reach
                        {
                            joins.add(
                                VectorPath::new()
                                    .move_to(x, centre(a.lane))
                                    .line_to(x, centre(b.lane)),
                            );
                        }
                    }
                    joins.stroke(c, ColorToken::BaseContent.alpha(22), 1.);
                }
                // One batch per lane (spans in a lane never overlap), and the
                // selected span on its own, at full strength.
                for (lane, color) in colors.iter().enumerate() {
                    let mut batch = PathBatch::new();
                    let mut chosen = None;
                    for (i, s) in draw_spans
                        .iter()
                        .enumerate()
                        .filter(|(_, s)| s.lane == lane)
                    {
                        let x0 = x_of(s.start).clamp(0., plot_w);
                        let x1 = x_of(s.end).clamp(0., plot_w).min(reach);
                        if x1 <= x0 {
                            continue;
                        }
                        let y = centre(lane) - bar / 2.;
                        let shape = VectorPath::rounded_rectangle(
                            x0,
                            y,
                            (x1 - x0).max(1.),
                            bar,
                            (bar * 0.3).min(4.),
                        );
                        if selected == Some(i) {
                            chosen = Some(shape);
                        } else {
                            batch.add(shape);
                        }
                    }
                    batch.fill(
                        c,
                        if selected.is_some() {
                            faded(color, 45)
                        } else {
                            color.clone()
                        },
                    );
                    if let Some(shape) = chosen {
                        c.fill(shape, color.clone());
                    }
                }
            },
        );
        let lane_labels: Vec<(f32, String)> = names
            .iter()
            .enumerate()
            .map(|(i, n)| ((i as f32 + 0.5) * lane_h, n.clone()))
            .collect();
        assemble(
            width,
            plot,
            [plot_w, plot_h],
            overlay.into_iter().collect(),
            lane_labels,
            spread_labels(&a.x_labels, plot_w, false),
            readout_line,
            None,
        )
    }
}
