//! Rotated and tilted views: where a point on the ground (or above it, for
//! buildings) lands on screen, and which point of the ground a screen point
//! shows.
//!
//! Positions on the ground use a frame turned with the map: x to the right
//! of the screen and y towards its bottom, in points at the current zoom,
//! from the point under the middle of the map. The camera hangs above that
//! point at 1.5 times the map's height (the field of view other map
//! renderers use) and leans back by the pitch, so at any pitch the middle
//! of the map keeps its scale.
use super::Camera;

/// The steepest tilt. Up to 60° the horizon stays above the top of the map,
/// so every point of the map shows ground.
pub const MAX_PITCH: f64 = 60.;

#[derive(Clone, Copy, Debug)]
pub struct View {
    /// The point under the middle of the map, in Web Mercator units.
    pub center: [f64; 2],
    /// The world's width in points.
    pub world: f64,
    pub size: [f64; 2],
    cos: f64,
    sin: f64,
    tilt_sin: f64,
    tilt_cos: f64,
    /// How far the camera is from the middle of the map, in points.
    pub distance: f64,
}

impl View {
    pub fn new(camera: Camera, size: [f32; 2]) -> Self {
        let bearing = camera.bearing.to_radians();
        let pitch = camera.pitch.clamp(0., MAX_PITCH).to_radians();
        let size = [f64::from(size[0].max(1.)), f64::from(size[1].max(1.))];
        Self {
            center: camera.center,
            world: camera.world(),
            size,
            cos: bearing.cos(),
            sin: bearing.sin(),
            tilt_sin: pitch.sin(),
            tilt_cos: pitch.cos(),
            distance: 1.5 * size[1],
        }
    }

    /// From a world offset (east, south) to the turned frame.
    pub fn turn(&self, d: [f64; 2]) -> [f64; 2] {
        [
            d[0] * self.cos + d[1] * self.sin,
            -d[0] * self.sin + d[1] * self.cos,
        ]
    }

    pub fn unturn(&self, r: [f64; 2]) -> [f64; 2] {
        [
            r[0] * self.cos - r[1] * self.sin,
            r[0] * self.sin + r[1] * self.cos,
        ]
    }

    /// Where a point `z` points above the ground lands on screen; `None`
    /// for points too near the camera to draw.
    pub fn project(&self, r: [f64; 2], z: f64) -> Option<[f64; 2]> {
        let depth = self.distance - r[1] * self.tilt_sin - z * self.tilt_cos;
        if depth < self.distance * 0.05 {
            return None;
        }
        let s = self.distance / depth;
        Some([
            self.size[0] / 2. + r[0] * s,
            self.size[1] / 2. + (r[1] * self.tilt_cos - z * self.tilt_sin) * s,
        ])
    }

    /// How much larger than in the middle the ground is drawn at `r`.
    pub fn scale(&self, r: [f64; 2]) -> f64 {
        self.distance / (self.distance - r[1] * self.tilt_sin).max(self.distance * 0.05)
    }

    /// The ground under a screen point, if that point shows ground.
    pub fn ground(&self, at: [f64; 2]) -> Option<[f64; 2]> {
        let (x, y) = (at[0] - self.size[0] / 2., at[1] - self.size[1] / 2.);
        let den = self.distance * self.tilt_cos + y * self.tilt_sin;
        if den <= 1e-9 {
            return None;
        }
        let gy = y * self.distance / den;
        let s = self.distance / (self.distance - gy * self.tilt_sin);
        Some([x / s, gy])
    }

    /// A Web Mercator point in the turned frame, taking the copy of the
    /// world nearest the middle of the map.
    pub fn turned_from_mercator(&self, p: [f64; 2]) -> [f64; 2] {
        let mut dx = p[0] - self.center[0];
        if dx > 0.5 {
            dx -= 1.;
        } else if dx < -0.5 {
            dx += 1.;
        }
        self.turn([dx * self.world, (p[1] - self.center[1]) * self.world])
    }

    /// The Web Mercator point at `r` (x not wrapped into 0..1).
    pub fn mercator_from_turned(&self, r: [f64; 2]) -> [f64; 2] {
        let d = self.unturn(r);
        [
            self.center[0] + d[0] / self.world,
            self.center[1] + d[1] / self.world,
        ]
    }

    /// A point where an untilted, unturned map would draw it (as tiles are
    /// placed) in the turned frame.
    pub fn turned_from_flat(&self, f: [f64; 2]) -> [f64; 2] {
        self.turn([f[0] - self.size[0] / 2., f[1] - self.size[1] / 2.])
    }

    /// Where the camera stands, in the turned frame (for buildings: which
    /// walls face it, and which are nearer).
    pub fn eye(&self) -> [f64; 2] {
        [0., self.distance * self.tilt_sin]
    }

    /// The ground in view, `margin` points beyond the map's edges, as a
    /// convex outline in the turned frame.
    pub fn visible(&self, margin: f64) -> Vec<[f64; 2]> {
        let [w, h] = self.size;
        let m = margin.min(h * 0.3);
        [[-m, -m], [w + m, -m], [w + m, h + m], [-m, h + m]]
            .into_iter()
            .filter_map(|at| self.ground(at))
            .collect()
    }
}

/// Which side of the line a→b a point is on (positive: left, in a frame
/// with y down that is the screen's right-hand turn).
fn side(a: [f64; 2], b: [f64; 2], p: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
}

/// Twice the signed area of a ring.
pub fn area(ring: &[[f64; 2]]) -> f64 {
    let n = ring.len();
    (0..n)
        .map(|i| {
            let (a, b) = (ring[i], ring[(i + 1) % n]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum()
}

/// Whether a point is inside a convex outline.
pub fn contains(outline: &[[f64; 2]], p: [f64; 2]) -> bool {
    let sign = area(outline).signum();
    outline.len() >= 3
        && (0..outline.len())
            .all(|i| side(outline[i], outline[(i + 1) % outline.len()], p) * sign >= 0.)
}

/// The part of a polygon ring inside a convex outline (Sutherland–Hodgman).
pub fn clip_polygon(ring: &[[f64; 2]], outline: &[[f64; 2]]) -> Vec<[f64; 2]> {
    if outline.len() < 3 {
        return Vec::new();
    }
    let sign = area(outline).signum();
    let mut points = ring.to_vec();
    for i in 0..outline.len() {
        if points.is_empty() {
            break;
        }
        let (a, b) = (outline[i], outline[(i + 1) % outline.len()]);
        let keep = |p: [f64; 2]| side(a, b, p) * sign >= 0.;
        let cross = |p: [f64; 2], q: [f64; 2]| {
            let (sp, sq) = (side(a, b, p), side(a, b, q));
            let t = sp / (sp - sq);
            [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t]
        };
        let mut out = Vec::with_capacity(points.len() + 2);
        for j in 0..points.len() {
            let (prev, cur) = (points[(j + points.len() - 1) % points.len()], points[j]);
            match (keep(prev), keep(cur)) {
                (true, true) => out.push(cur),
                (true, false) => out.push(cross(prev, cur)),
                (false, true) => {
                    out.push(cross(prev, cur));
                    out.push(cur);
                }
                (false, false) => {}
            }
        }
        points = out;
    }
    points
}

/// The visible pieces of a line inside a convex outline (Cyrus–Beck).
pub fn clip_polyline(line: &[[f64; 2]], outline: &[[f64; 2]]) -> Vec<Vec<[f64; 2]>> {
    if outline.len() < 3 {
        return Vec::new();
    }
    let sign = area(outline).signum();
    let segment = |p: [f64; 2], q: [f64; 2]| -> Option<([f64; 2], [f64; 2])> {
        let (mut t0, mut t1) = (0f64, 1f64);
        for i in 0..outline.len() {
            let (a, b) = (outline[i], outline[(i + 1) % outline.len()]);
            let (sp, sq) = (side(a, b, p) * sign, side(a, b, q) * sign);
            if sp < 0. && sq < 0. {
                return None;
            }
            if sp < 0. {
                t0 = t0.max(sp / (sp - sq));
            } else if sq < 0. {
                t1 = t1.min(sp / (sp - sq));
            }
            if t0 > t1 {
                return None;
            }
        }
        let at = |t: f64| [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t];
        Some((at(t0), at(t1)))
    };
    let mut pieces = Vec::new();
    let mut current: Vec<[f64; 2]> = Vec::new();
    for w in line.windows(2) {
        match segment(w[0], w[1]) {
            Some((a, b)) => {
                let joined = current
                    .last()
                    .is_some_and(|l| (l[0] - a[0]).abs() < 1e-6 && (l[1] - a[1]).abs() < 1e-6);
                if !joined {
                    if current.len() >= 2 {
                        pieces.push(std::mem::take(&mut current));
                    }
                    current = vec![a];
                }
                current.push(b);
            }
            None => {
                if current.len() >= 2 {
                    pieces.push(std::mem::take(&mut current));
                }
                current.clear();
            }
        }
    }
    if current.len() >= 2 {
        pieces.push(current);
    }
    pieces
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f64; 2], b: [f64; 2]) -> bool {
        (a[0] - b[0]).abs() < 1e-6 && (a[1] - b[1]).abs() < 1e-6
    }

    #[test]
    fn screen_and_ground_agree_at_any_bearing_and_pitch() {
        for (bearing, pitch) in [(0., 0.), (30., 0.), (0., 45.), (-120., 60.), (200., 20.)] {
            let camera = Camera {
                center: [0.3, 0.4],
                zoom: 12.,
                bearing,
                pitch,
            };
            let view = View::new(camera, [1280., 800.]);
            for at in [
                [640., 400.],
                [0., 0.],
                [1280., 800.],
                [100., 700.],
                [1100., 50.],
            ] {
                let ground = view
                    .ground(at)
                    .expect("every point of the map shows ground");
                let back = view.project(ground, 0.).unwrap();
                assert!(
                    close(back, at),
                    "{bearing}/{pitch}: {at:?} → {ground:?} → {back:?}"
                );
            }
            // The middle keeps its scale.
            assert!(close(view.project([10., 0.], 0.).unwrap(), [650., 400.]));
            let p = [0.3004, 0.3998];
            assert!(close(
                view.mercator_from_turned(view.turned_from_mercator(p)),
                p
            ));
        }
    }

    #[test]
    fn a_bearing_turns_the_map_clockwise_under_the_camera() {
        // Facing east, a place to the east is at the top of the screen.
        let camera = Camera {
            center: [0.5, 0.5],
            zoom: 10.,
            bearing: 90.,
            pitch: 0.,
        };
        let view = View::new(camera, [800., 600.]);
        let east = view.turned_from_mercator([0.5 + 100. / view.world, 0.5]);
        let at = view.project(east, 0.).unwrap();
        assert!(close(at, [400., 200.]), "{at:?}");
    }

    #[test]
    fn tilting_shrinks_the_far_side_and_lifts_tall_things() {
        let camera = Camera {
            center: [0.5, 0.5],
            zoom: 14.,
            bearing: 0.,
            pitch: 50.,
        };
        let view = View::new(camera, [800., 600.]);
        assert!(view.scale([0., -300.]) < 1. && view.scale([0., 200.]) > 1.);
        let base = view.project([0., 100.], 0.).unwrap();
        let top = view.project([0., 100.], 80.).unwrap();
        assert!(top[1] < base[1], "a roof is drawn above its base");
    }

    #[test]
    fn shapes_are_cut_to_a_turned_outline() {
        let diamond = [[0., -10.], [10., 0.], [0., 10.], [-10., 0.]];
        let square = [[-20., -20.], [0., -20.], [0., 20.], [-20., 20.]];
        let cut = clip_polygon(&square, &diamond);
        assert!(
            (area(&cut).abs() / 2. - 100.).abs() < 1e-6,
            "half the diamond: {cut:?}"
        );
        let pieces = clip_polyline(&[[-30., 0.], [30., 0.]], &diamond);
        assert_eq!(pieces.len(), 1);
        assert!(close(pieces[0][0], [-10., 0.]) && close(pieces[0][1], [10., 0.]));
    }
}
