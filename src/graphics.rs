//! Renderer-independent image handles, vector paths and explicit clip geometry.
//! Assets are supplied by the host; this module never reads files or the network.
use std::sync::Arc;

/// Immutable premultiplied RGBA pixels. Clones share storage; at most 16 MP.
#[derive(Clone, Debug)]
pub struct Image(Arc<ImageData>);
#[derive(Debug)]
struct ImageData {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}
impl PartialEq for Image {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Image {
    /// Input is straight RGBA; conversion happens once, never during painting.
    pub fn from_rgba(width: u32, height: u32, mut pixels: Vec<u8>) -> Result<Self, &'static str> {
        let count = u64::from(width) * u64::from(height);
        if count == 0 || count > 16_777_216 || count * 4 != pixels.len() as u64 {
            return Err("invalid image dimensions/data (16 MP maximum)");
        }
        for p in pixels.chunks_exact_mut(4) {
            for i in 0..3 {
                p[i] = ((u16::from(p[i]) * u16::from(p[3]) + 127) / 255) as u8;
            }
        }
        Ok(Self(Arc::new(ImageData {
            width,
            height,
            pixels,
        })))
    }
    pub fn width(&self) -> u32 {
        self.0.width
    }
    pub fn height(&self) -> u32 {
        self.0.height
    }
    /// Premultiplied RGBA, suitable for renderer upload without decoding.
    pub fn pixels(&self) -> &[u8] {
        &self.0.pixels
    }
    pub fn background(self) -> BackgroundImage {
        BackgroundImage {
            image: self,
            fit: ImageFit::Cover,
            position: ImagePosition::Center,
            opacity: 1.,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImageFit {
    #[default]
    Cover,
    Contain,
    Stretch,
    Tile,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImagePosition {
    #[default]
    Center,
    Top,
    Right,
    Bottom,
    Left,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}
impl ImagePosition {
    pub fn fractions(self) -> [f32; 2] {
        match self {
            Self::Center => [0.5, 0.5],
            Self::Top => [0.5, 0.],
            Self::Right => [1., 0.5],
            Self::Bottom => [0.5, 1.],
            Self::Left => [0., 0.5],
            Self::TopLeft => [0., 0.],
            Self::TopRight => [1., 0.],
            Self::BottomLeft => [0., 1.],
            Self::BottomRight => [1., 1.],
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct BackgroundImage {
    pub image: Image,
    pub fit: ImageFit,
    pub position: ImagePosition,
    pub opacity: f32,
}
impl From<Image> for BackgroundImage {
    fn from(value: Image) -> Self {
        value.background()
    }
}
impl BackgroundImage {
    pub fn fit(mut self, fit: ImageFit) -> Self {
        self.fit = fit;
        self
    }
    pub fn position(mut self, position: ImagePosition) -> Self {
        self.position = position;
        self
    }
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum PathCommand {
    Move([f32; 2]),
    Line([f32; 2]),
    Quad([f32; 2], [f32; 2]),
    Cubic([f32; 2], [f32; 2], [f32; 2]),
    Close,
}
/// Paths use local logical coordinates. Fills/clips use the even-odd rule.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VectorPath {
    pub commands: Vec<PathCommand>,
}
impl VectorPath {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn move_to(mut self, x: f32, y: f32) -> Self {
        self.commands.push(PathCommand::Move([x, y]));
        self
    }
    pub fn line_to(mut self, x: f32, y: f32) -> Self {
        self.commands.push(PathCommand::Line([x, y]));
        self
    }
    pub fn quad_to(mut self, control: [f32; 2], end: [f32; 2]) -> Self {
        self.commands.push(PathCommand::Quad(control, end));
        self
    }
    pub fn cubic_to(mut self, a: [f32; 2], b: [f32; 2], end: [f32; 2]) -> Self {
        self.commands.push(PathCommand::Cubic(a, b, end));
        self
    }
    pub fn close(mut self) -> Self {
        self.commands.push(PathCommand::Close);
        self
    }
    pub fn rectangle(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self::new()
            .move_to(x, y)
            .line_to(x + w, y)
            .line_to(x + w, y + h)
            .line_to(x, y + h)
            .close()
    }
    pub fn circle(x: f32, y: f32, r: f32) -> Self {
        Self::ellipse(x, y, r, r)
    }
    pub fn ellipse(x: f32, y: f32, rx: f32, ry: f32) -> Self {
        let k = 0.552_284_8;
        Self::new()
            .move_to(x + rx, y)
            .cubic_to([x + rx, y + k * ry], [x + k * rx, y + ry], [x, y + ry])
            .cubic_to([x - k * rx, y + ry], [x - rx, y + k * ry], [x - rx, y])
            .cubic_to([x - rx, y - k * ry], [x - k * rx, y - ry], [x, y - ry])
            .cubic_to([x + k * rx, y - ry], [x + rx, y - k * ry], [x + rx, y])
            .close()
    }
    /// Affine matrix `[a, b, c, d, tx, ty]`. Can translate/scale/rotate paths in Rust.
    pub fn transformed(mut self, m: [f32; 6]) -> Self {
        let point = |p: &mut [f32; 2]| {
            let [x, y] = *p;
            *p = [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]];
        };
        for c in &mut self.commands {
            match c {
                PathCommand::Move(p) | PathCommand::Line(p) => point(p),
                PathCommand::Quad(a, b) => {
                    point(a);
                    point(b)
                }
                PathCommand::Cubic(a, b, c) => {
                    point(a);
                    point(b);
                    point(c)
                }
                PathCommand::Close => {}
            }
        }
        self
    }
    pub fn is_valid(&self) -> bool {
        let valid = |p: &[f32; 2]| p.iter().all(|v| v.is_finite() && v.abs() <= 65_536.);
        !self.commands.is_empty()
            && self.commands.iter().any(|c| {
                matches!(
                    c,
                    PathCommand::Line(_) | PathCommand::Quad(..) | PathCommand::Cubic(..)
                )
            })
            && self.commands.len() <= 1024
            && matches!(self.commands[0], PathCommand::Move(_))
            && self.commands.iter().all(|c| match c {
                PathCommand::Move(p) | PathCommand::Line(p) => valid(p),
                PathCommand::Quad(a, b) => valid(a) && valid(b),
                PathCommand::Cubic(a, b, c) => valid(a) && valid(b) && valid(c),
                PathCommand::Close => true,
            })
    }
    /// Even-odd interior. Curves are flattened at bounded resolution for input.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        let mut inside = false;
        let mut prev = [0.; 2];
        let mut start = prev;
        let mut open = false;
        let edge = |a: [f32; 2], b: [f32; 2], inside: &mut bool| {
            if (a[1] > y) != (b[1] > y) && x < (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]) + a[0] {
                *inside = !*inside;
            }
        };
        for c in &self.commands {
            match *c {
                PathCommand::Move(p) => {
                    if open {
                        edge(prev, start, &mut inside)
                    };
                    prev = p;
                    start = p;
                    open = true
                }
                PathCommand::Line(p) => {
                    edge(prev, p, &mut inside);
                    prev = p
                }
                PathCommand::Quad(a, b) => {
                    let from = prev;
                    for i in 1..=32 {
                        let t = i as f32 / 32.;
                        let u = 1. - t;
                        let p = [
                            u * u * from[0] + 2. * u * t * a[0] + t * t * b[0],
                            u * u * from[1] + 2. * u * t * a[1] + t * t * b[1],
                        ];
                        edge(prev, p, &mut inside);
                        prev = p
                    }
                }
                PathCommand::Cubic(a, b, c) => {
                    let from = prev;
                    for i in 1..=32 {
                        let t = i as f32 / 32.;
                        let u = 1. - t;
                        let p = [
                            u * u * u * from[0]
                                + 3. * u * u * t * a[0]
                                + 3. * u * t * t * b[0]
                                + t * t * t * c[0],
                            u * u * u * from[1]
                                + 3. * u * u * t * a[1]
                                + 3. * u * t * t * b[1]
                                + t * t * t * c[1],
                        ];
                        edge(prev, p, &mut inside);
                        prev = p
                    }
                }
                PathCommand::Close => {
                    edge(prev, start, &mut inside);
                    prev = start;
                    open = false
                }
            }
        }
        if open {
            edge(prev, start, &mut inside)
        };
        inside
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum Clip {
    Bounds,
    Rounded,
    Circle,
    Path(VectorPath),
}
/// Resolved physical-pixel clip shared by rendering and interaction.
#[derive(Clone, Debug, PartialEq)]
pub enum ClipRegion {
    Rounded { bounds: [f32; 4], radii: [f32; 4] },
    Circle { bounds: [f32; 4] },
    Path(VectorPath),
}
impl ClipRegion {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        match self {
            Self::Rounded { bounds, radii } => {
                crate::Rect::new(bounds[0], bounds[1], bounds[2], bounds[3])
                    .is_some_and(|r| r.contains_corners(*radii, x, y))
            }
            Self::Circle {
                bounds: [bx, by, w, h],
            } => {
                let r = w.min(*h) * 0.5;
                (x - (bx + w * 0.5)).hypot(y - (by + h * 0.5)) < r
            }
            Self::Path(p) => p.contains(x, y),
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct DrawPath {
    pub path: VectorPath,
    pub color: String,
    pub stroke: Option<f32>,
}
/// A retained list built by an ordinary Rust closure when the view composes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Canvas {
    pub paths: Vec<DrawPath>,
}
impl Canvas {
    pub fn fill(&mut self, path: VectorPath, color: impl Into<String>) {
        self.paths.push(DrawPath {
            path,
            color: color.into(),
            stroke: None,
        });
    }
    pub fn stroke(&mut self, path: VectorPath, color: impl Into<String>, width: f32) {
        self.paths.push(DrawPath {
            path,
            color: color.into(),
            stroke: Some(width),
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_handles_share_storage_and_premultiply_once() {
        let image = Image::from_rgba(1, 1, vec![255, 128, 0, 128]).unwrap();
        assert_eq!(image.pixels(), [128, 64, 0, 128]);
        assert_eq!(image, image.clone());
        assert!(Image::from_rgba(u32::MAX, u32::MAX, vec![]).is_err());
        assert!(Image::from_rgba(2, 2, vec![0; 4]).is_err());
    }
    #[test]
    fn paths_transform_even_odd_holes_and_validate_finite_commands() {
        let mut path = VectorPath::rectangle(0., 0., 100., 100.);
        path.commands
            .extend(VectorPath::rectangle(20., 20., 60., 60.).commands);
        assert!(path.contains(10., 10.));
        assert!(!path.contains(50., 50.));
        let path = path.transformed([2., 0., 0., 2., 10., 20.]);
        assert!(path.contains(30., 40.));
        assert!(!path.contains(110., 120.));
        assert!(!VectorPath::new().move_to(0., 0.).is_valid());
        assert!(!VectorPath::circle(0., 0., f32::NAN).is_valid());
        assert!(VectorPath::circle(50., 50., 25.).contains(50., 50.));
    }
}
