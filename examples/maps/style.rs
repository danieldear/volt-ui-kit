//! A practical subset of the open map Style Specification (the `style.json`
//! format used by Mapbox, MapLibre, OpenFreeMap, MapTiler and others):
//! sources, layers, legacy filters and the common expressions.
use super::mvt::{Feature, Geometry, Value as Prop};
use serde_json::{Map, Value as Json};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    Vector,
    Raster,
}

#[derive(Clone, Debug)]
pub struct Source {
    pub kind: SourceKind,
    /// A TileJSON document to fetch for `tiles`, when they aren't inline.
    pub url: Option<String>,
    pub tiles: Vec<String>,
    pub tile_size: f64,
    pub min_zoom: u8,
    pub max_zoom: u8,
    pub attribution: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerKind {
    Background,
    Fill,
    Line,
    Symbol,
    Raster,
    Extrusion,
    Other,
}

#[derive(Clone, Debug)]
pub struct Layer {
    pub id: String,
    pub kind: LayerKind,
    pub source: Option<String>,
    pub source_layer: Option<String>,
    pub min_zoom: f64,
    pub max_zoom: f64,
    pub filter: Option<Json>,
    pub paint: Map<String, Json>,
    pub layout: Map<String, Json>,
}

#[derive(Clone, Debug, Default)]
pub struct Style {
    pub sources: BTreeMap<String, Source>,
    pub layers: Vec<Layer>,
}

impl Style {
    pub fn parse(text: &str) -> Result<Self, String> {
        let json: Json = serde_json::from_str(text).map_err(|e| format!("style: {e}"))?;
        let mut style = Style::default();
        for (id, s) in json["sources"].as_object().ok_or("style has no sources")? {
            let kind = match s["type"].as_str() {
                Some("vector") => SourceKind::Vector,
                Some("raster") => SourceKind::Raster,
                _ => continue,
            };
            let mut source = Source {
                kind,
                url: s["url"].as_str().map(str::to_owned),
                tiles: Vec::new(),
                tile_size: s["tileSize"].as_f64().unwrap_or(match kind {
                    SourceKind::Vector => 512.,
                    SourceKind::Raster => 512.,
                }),
                min_zoom: 0,
                max_zoom: 22,
                attribution: None,
            };
            source.apply_tilejson(s);
            style.sources.insert(id.clone(), source);
        }
        for l in json["layers"].as_array().ok_or("style has no layers")? {
            let layout = l["layout"].as_object().cloned().unwrap_or_default();
            if layout.get("visibility").and_then(Json::as_str) == Some("none") {
                continue;
            }
            let kind = match l["type"].as_str().unwrap_or("") {
                "background" => LayerKind::Background,
                "fill" => LayerKind::Fill,
                "line" => LayerKind::Line,
                "symbol" => LayerKind::Symbol,
                "raster" => LayerKind::Raster,
                "fill-extrusion" => LayerKind::Extrusion,
                _ => LayerKind::Other,
            };
            style.layers.push(Layer {
                id: l["id"].as_str().unwrap_or("").to_owned(),
                kind,
                source: l["source"].as_str().map(str::to_owned),
                source_layer: l["source-layer"].as_str().map(str::to_owned),
                min_zoom: l["minzoom"].as_f64().unwrap_or(0.),
                max_zoom: l["maxzoom"].as_f64().unwrap_or(24.),
                filter: l.get("filter").cloned(),
                paint: l["paint"].as_object().cloned().unwrap_or_default(),
                layout,
            });
        }
        Ok(style)
    }
}

impl Source {
    /// Take tiles, zoom range and attribution from inline fields or a TileJSON.
    pub fn apply_tilejson(&mut self, j: &Json) {
        if let Some(tiles) = j["tiles"].as_array() {
            self.tiles = tiles
                .iter()
                .filter_map(Json::as_str)
                .map(str::to_owned)
                .collect();
        }
        if let Some(z) = j["minzoom"].as_u64() {
            self.min_zoom = z.min(24) as u8;
        }
        if let Some(z) = j["maxzoom"].as_u64() {
            self.max_zoom = z.min(24) as u8;
        }
        if let Some(a) = j["attribution"].as_str() {
            self.attribution = Some(a.to_owned());
        }
    }
}

// ── Expressions ────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum V {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Color([f32; 4]),
    Arr(Vec<V>),
}

impl V {
    fn truthy(&self) -> bool {
        match self {
            V::Null => false,
            V::Bool(b) => *b,
            V::Num(n) => *n != 0.,
            V::Str(s) => !s.is_empty(),
            _ => true,
        }
    }
    pub fn num(&self) -> Option<f64> {
        match self {
            V::Num(n) => Some(*n),
            V::Str(s) => s.parse().ok(),
            V::Bool(b) => Some(f64::from(u8::from(*b))),
            _ => None,
        }
    }
    pub fn color(&self) -> Option<[f32; 4]> {
        match self {
            V::Color(c) => Some(*c),
            V::Str(s) => parse_color(s),
            _ => None,
        }
    }
    pub fn text(&self) -> String {
        match self {
            V::Null => String::new(),
            V::Bool(b) => b.to_string(),
            V::Num(n) if n.fract() == 0. && n.abs() < 1e15 => format!("{}", *n as i64),
            V::Num(n) => n.to_string(),
            V::Str(s) => s.clone(),
            V::Color(c) => format!("{c:?}"),
            V::Arr(items) => items.iter().map(V::text).collect(),
        }
    }
}

/// What an expression can read: the camera zoom and the feature, if any.
pub struct Ctx<'a> {
    pub zoom: f64,
    pub feature: Option<&'a Feature>,
}

impl Ctx<'_> {
    fn property(&self, key: &str) -> V {
        if key == "$type" {
            return V::Str(self.base_type().into());
        }
        match self.feature.and_then(|f| f.properties.get(key)) {
            Some(Prop::Str(s)) => V::Str(s.clone()),
            Some(Prop::Num(n)) => V::Num(*n),
            Some(Prop::Bool(b)) => V::Bool(*b),
            None => V::Null,
        }
    }
    fn base_type(&self) -> &'static str {
        match self.feature.map(|f| f.kind) {
            Some(Geometry::Point) => "Point",
            Some(Geometry::Line) => "LineString",
            Some(Geometry::Polygon) => "Polygon",
            None => "",
        }
    }
    fn geometry_type(&self) -> String {
        let Some(f) = self.feature else {
            return String::new();
        };
        let multi = match f.kind {
            Geometry::Polygon => {
                let sign = |r: &Vec<[f32; 2]>| ring_area(r) > 0.;
                let first = f.parts.first().map(sign);
                f.parts.iter().filter(|r| Some(sign(r)) == first).count() > 1
            }
            _ => f.parts.len() > 1,
        };
        format!("{}{}", if multi { "Multi" } else { "" }, self.base_type())
    }
}

pub fn ring_area(ring: &[[f32; 2]]) -> f32 {
    ring.windows(2)
        .map(|w| w[0][0] * w[1][1] - w[1][0] * w[0][1])
        .sum::<f32>()
}

/// A style filter: legacy (`["==", "class", "park"]`) or expression form.
pub fn filter(f: &Json, ctx: &Ctx) -> bool {
    if legacy(f) {
        legacy_filter(f, ctx)
    } else {
        eval(f, ctx).truthy()
    }
}

fn legacy(f: &Json) -> bool {
    let Some(a) = f.as_array() else {
        return false;
    };
    match a.first().and_then(Json::as_str) {
        Some("==" | "!=" | "<" | "<=" | ">" | ">=" | "in" | "!in") => {
            a.get(1).is_some_and(Json::is_string)
        }
        Some("!has") => true,
        Some("has") => a.len() == 2 && a[1].is_string(),
        Some("all" | "any" | "none") => a[1..].iter().all(|c| legacy(c) || c.is_boolean()),
        _ => false,
    }
}

fn legacy_filter(f: &Json, ctx: &Ctx) -> bool {
    let a = f.as_array().unwrap();
    let op = a[0].as_str().unwrap_or("");
    let key = || a[1].as_str().unwrap_or("");
    let lit = |j: &Json| literal(j);
    match op {
        "all" => a[1..].iter().all(|c| legacy_filter_or_bool(c, ctx)),
        "any" => a[1..].iter().any(|c| legacy_filter_or_bool(c, ctx)),
        "none" => !a[1..].iter().any(|c| legacy_filter_or_bool(c, ctx)),
        "has" => !matches!(ctx.property(key()), V::Null),
        "!has" => matches!(ctx.property(key()), V::Null),
        "in" => {
            let v = ctx.property(key());
            a[2..].iter().any(|j| lit(j) == v)
        }
        "!in" => {
            let v = ctx.property(key());
            !a[2..].iter().any(|j| lit(j) == v)
        }
        _ => compare(
            op,
            &ctx.property(key()),
            &a.get(2).map(lit).unwrap_or(V::Null),
        ),
    }
}

fn legacy_filter_or_bool(f: &Json, ctx: &Ctx) -> bool {
    f.as_bool().unwrap_or_else(|| legacy_filter(f, ctx))
}

fn literal(j: &Json) -> V {
    match j {
        Json::Null => V::Null,
        Json::Bool(b) => V::Bool(*b),
        Json::Number(n) => V::Num(n.as_f64().unwrap_or(0.)),
        Json::String(s) => V::Str(s.clone()),
        Json::Array(items) => V::Arr(items.iter().map(literal).collect()),
        Json::Object(_) => V::Null,
    }
}

fn compare(op: &str, a: &V, b: &V) -> bool {
    use std::cmp::Ordering;
    let order = match (a, b) {
        (V::Num(x), V::Num(y)) => x.partial_cmp(y),
        (V::Str(x), V::Str(y)) => Some(x.cmp(y)),
        _ => None,
    };
    match op {
        "==" => a == b,
        "!=" => a != b,
        "<" => order == Some(Ordering::Less),
        "<=" => matches!(order, Some(Ordering::Less | Ordering::Equal)),
        ">" => order == Some(Ordering::Greater),
        ">=" => matches!(order, Some(Ordering::Greater | Ordering::Equal)),
        _ => false,
    }
}

/// Evaluate a property value or expression.
pub fn eval(j: &Json, ctx: &Ctx) -> V {
    match j {
        Json::Array(a) if a.first().is_some_and(Json::is_string) => op(a, ctx),
        Json::Object(o) if o.contains_key("stops") => function(o, ctx),
        other => literal(other),
    }
}

fn op(a: &[Json], ctx: &Ctx) -> V {
    let name = a[0].as_str().unwrap();
    let arg = |i: usize| a.get(i).map(|j| eval(j, ctx)).unwrap_or(V::Null);
    match name {
        "literal" => a.get(1).map(literal).unwrap_or(V::Null),
        "get" => match arg(1) {
            V::Str(k) => ctx.property(&k),
            _ => V::Null,
        },
        "has" => match arg(1) {
            V::Str(k) => V::Bool(!matches!(ctx.property(&k), V::Null)),
            _ => V::Bool(false),
        },
        "!has" => match arg(1) {
            V::Str(k) => V::Bool(matches!(ctx.property(&k), V::Null)),
            _ => V::Bool(true),
        },
        "zoom" => V::Num(ctx.zoom),
        "geometry-type" => V::Str(ctx.geometry_type()),
        "==" | "!=" | "<" | "<=" | ">" | ">=" => V::Bool(compare(name, &arg(1), &arg(2))),
        "all" => V::Bool((1..a.len()).all(|i| arg(i).truthy())),
        "any" => V::Bool((1..a.len()).any(|i| arg(i).truthy())),
        "none" => V::Bool(!(1..a.len()).any(|i| arg(i).truthy())),
        "!" => V::Bool(!arg(1).truthy()),
        "in" => {
            let (needle, hay) = (arg(1), arg(2));
            V::Bool(match hay {
                V::Arr(items) => items.contains(&needle),
                V::Str(s) => s.contains(&needle.text()),
                _ => false,
            })
        }
        "match" => {
            let input = arg(1);
            let mut i = 2;
            while i + 1 < a.len() {
                let hit = match &a[i] {
                    Json::Array(labels) => labels.iter().any(|l| literal(l) == input),
                    label => literal(label) == input,
                };
                if hit {
                    return arg(i + 1);
                }
                i += 2;
            }
            arg(a.len() - 1)
        }
        "case" => {
            let mut i = 1;
            while i + 1 < a.len() {
                if arg(i).truthy() {
                    return arg(i + 1);
                }
                i += 2;
            }
            arg(a.len() - 1)
        }
        "coalesce" => (1..a.len())
            .map(arg)
            .find(|v| !matches!(v, V::Null))
            .unwrap_or(V::Null),
        "interpolate" => {
            let base = match a.get(1).and_then(Json::as_array).map(Vec::as_slice) {
                Some([kind, b]) if kind == "exponential" => b.as_f64().unwrap_or(1.),
                _ => 1.,
            };
            let input = arg(2).num().unwrap_or(0.);
            let stops: Vec<(f64, V)> = a[3..]
                .chunks_exact(2)
                .map(|p| (p[0].as_f64().unwrap_or(0.), eval(&p[1], ctx)))
                .collect();
            interpolate(&stops, input, base)
        }
        "step" => {
            let input = arg(1).num().unwrap_or(0.);
            let mut out = arg(2);
            let mut i = 3;
            while i + 1 < a.len() {
                if input >= a[i].as_f64().unwrap_or(f64::MAX) {
                    out = arg(i + 1);
                }
                i += 2;
            }
            out
        }
        "concat" => V::Str((1..a.len()).map(|i| arg(i).text()).collect()),
        "format" => V::Str(
            (1..a.len())
                .filter(|&i| !a[i].is_object())
                .map(|i| arg(i).text())
                .collect(),
        ),
        "upcase" => V::Str(arg(1).text().to_uppercase()),
        "downcase" => V::Str(arg(1).text().to_lowercase()),
        "to-string" | "string" => V::Str(arg(1).text()),
        "to-number" | "number" => arg(1).num().map_or(V::Null, V::Num),
        "to-boolean" | "boolean" => V::Bool(arg(1).truthy()),
        "to-color" => arg(1).color().map_or(V::Null, V::Color),
        "typeof" => V::Str(
            match arg(1) {
                V::Null => "null",
                V::Bool(_) => "boolean",
                V::Num(_) => "number",
                V::Str(_) => "string",
                V::Color(_) => "color",
                V::Arr(_) => "array",
            }
            .into(),
        ),
        "rgb" | "rgba" => {
            let n = |i| arg(i).num().unwrap_or(0.) as f32;
            let alpha = if name == "rgba" { n(4) } else { 1. };
            V::Color([n(1) / 255., n(2) / 255., n(3) / 255., alpha])
        }
        "+" => V::Num((1..a.len()).filter_map(|i| arg(i).num()).sum()),
        "*" => V::Num((1..a.len()).filter_map(|i| arg(i).num()).product()),
        "-" => V::Num(arg(1).num().unwrap_or(0.) - arg(2).num().unwrap_or(0.)),
        "/" => V::Num(arg(1).num().unwrap_or(0.) / arg(2).num().unwrap_or(1.)),
        "image" => arg(1),
        _ => V::Null,
    }
}

/// Legacy `{"stops": [...]}` functions over zoom or a feature property.
fn function(o: &Map<String, Json>, ctx: &Ctx) -> V {
    let input = match o.get("property").and_then(Json::as_str) {
        Some(key) => ctx.property(key),
        None => V::Num(ctx.zoom),
    };
    let stops = o["stops"].as_array().cloned().unwrap_or_default();
    let pairs: Vec<(V, V)> = stops
        .iter()
        .filter_map(|s| Some((literal(s.get(0)?), eval(s.get(1)?, ctx))))
        .collect();
    let kind = o.get("type").and_then(Json::as_str).unwrap_or(match input {
        V::Str(_) => "categorical",
        _ => "exponential",
    });
    match kind {
        "categorical" => pairs
            .iter()
            .find(|(k, _)| *k == input)
            .map(|(_, v)| v.clone())
            .unwrap_or(V::Null),
        "interval" => {
            let x = input.num().unwrap_or(0.);
            pairs
                .iter()
                .rev()
                .find(|(k, _)| k.num().is_some_and(|k| x >= k))
                .or(pairs.first())
                .map(|(_, v)| v.clone())
                .unwrap_or(V::Null)
        }
        _ => {
            let base = o.get("base").and_then(Json::as_f64).unwrap_or(1.);
            let stops: Vec<(f64, V)> = pairs
                .into_iter()
                .filter_map(|(k, v)| Some((k.num()?, v)))
                .collect();
            interpolate(&stops, input.num().unwrap_or(0.), base)
        }
    }
}

fn interpolate(stops: &[(f64, V)], x: f64, base: f64) -> V {
    let Some(first) = stops.first() else {
        return V::Null;
    };
    if x <= first.0 {
        return first.1.clone();
    }
    for w in stops.windows(2) {
        let ((x0, v0), (x1, v1)) = (&w[0], &w[1]);
        if x <= *x1 {
            let span = x1 - x0;
            let t = if span <= 0. {
                0.
            } else if (base - 1.).abs() < 1e-9 {
                (x - x0) / span
            } else {
                (base.powf(x - x0) - 1.) / (base.powf(span) - 1.)
            };
            return mix(v0, v1, t);
        }
    }
    stops.last().unwrap().1.clone()
}

fn mix(a: &V, b: &V, t: f64) -> V {
    if let (Some(x), Some(y)) = (a.num(), b.num()) {
        if !matches!(a, V::Str(_)) {
            return V::Num(x + (y - x) * t);
        }
    }
    if let (Some(x), Some(y)) = (a.color(), b.color()) {
        let t = t as f32;
        return V::Color(std::array::from_fn(|i| x[i] + (y[i] - x[i]) * t));
    }
    if let (V::Arr(x), V::Arr(y)) = (a, b) {
        return V::Arr(x.iter().zip(y).map(|(p, q)| mix(p, q, t)).collect());
    }
    if t < 0.5 {
        a.clone()
    } else {
        b.clone()
    }
}

/// CSS-style colours: hex, rgb(a), hsl(a) and a few names.
pub fn parse_color(s: &str) -> Option<[f32; 4]> {
    let s = s.trim();
    if s.starts_with('#') {
        return volt_ui_kit::theme::parse_hex(s);
    }
    let named = match s {
        "white" => Some([1., 1., 1., 1.]),
        "black" => Some([0., 0., 0., 1.]),
        "transparent" => Some([0., 0., 0., 0.]),
        _ => None,
    };
    if named.is_some() {
        return named;
    }
    let open = s.find('(')?;
    let name = &s[..open];
    let args: Vec<&str> = s[open + 1..s.rfind(')')?]
        .split(',')
        .map(str::trim)
        .collect();
    let n = |i: usize| {
        args.get(i)
            .and_then(|v| v.trim_end_matches('%').parse::<f32>().ok())
    };
    match name {
        "rgb" | "rgba" => Some([n(0)? / 255., n(1)? / 255., n(2)? / 255., n(3).unwrap_or(1.)]),
        "hsl" | "hsla" => {
            let (h, sat, l) = (n(0)? / 360., n(1)? / 100., n(2)? / 100.);
            let q = if l < 0.5 {
                l * (1. + sat)
            } else {
                l + sat - l * sat
            };
            let p = 2. * l - q;
            let channel = |t: f32| {
                let t = t.rem_euclid(1.);
                if t < 1. / 6. {
                    p + (q - p) * 6. * t
                } else if t < 0.5 {
                    q
                } else if t < 2. / 3. {
                    p + (q - p) * (2. / 3. - t) * 6.
                } else {
                    p
                }
            };
            Some([
                channel(h + 1. / 3.),
                channel(h),
                channel(h - 1. / 3.),
                n(3).unwrap_or(1.),
            ])
        }
        _ => None,
    }
}

/// `#RRGGBBAA` for the kit's colour strings, with an extra opacity factor.
pub fn hex(c: [f32; 4], opacity: f32) -> String {
    let b = |v: f32| (v.clamp(0., 1.) * 255.).round() as u8;
    format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        b(c[0]),
        b(c[1]),
        b(c[2]),
        b(c[3] * opacity)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn expressions_cover_zoom_interpolation_match_and_legacy_filters() {
        let ctx = Ctx {
            zoom: 12.,
            feature: None,
        };
        let width = json!(["interpolate", ["exponential", 1.2], ["zoom"], 10, 1, 14, 4]);
        let w = eval(&width, &ctx).num().unwrap();
        assert!(w > 1. && w < 2.5, "{w}");
        assert_eq!(
            eval(&json!(["match", "a", ["a", "b"], 1, 2]), &ctx),
            V::Num(1.)
        );
        let red = parse_color("hsl(0, 100%, 50%)").unwrap();
        assert!(red
            .iter()
            .zip([1., 0., 0., 1.])
            .all(|(a, b)| (a - b).abs() < 1e-4));
        assert!(legacy(&json!(["==", "class", "park"])));
        assert!(!legacy(&json!(["==", ["get", "class"], "park"])));
        assert!(filter(&json!(["all", ["!has", "name"]]), &ctx));
    }
}
