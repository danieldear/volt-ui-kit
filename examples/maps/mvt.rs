//! A small decoder for Mapbox Vector Tiles (the open MVT 2.1 format that
//! Mapbox, OpenMapTiles, OpenFreeMap and others serve). Protobuf is decoded by
//! hand: the format uses only a handful of fields.
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Str(String),
    Num(f64),
    Bool(bool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Geometry {
    Point,
    Line,
    Polygon,
}

/// Geometry in tile units (0..extent), already decoded from commands.
#[derive(Clone, Debug)]
pub struct Feature {
    pub kind: Geometry,
    /// Points, line strings or polygon rings, each a list of [x, y].
    pub parts: Vec<Vec<[f32; 2]>>,
    pub properties: HashMap<String, Value>,
}

#[derive(Clone, Debug)]
pub struct Layer {
    pub extent: f32,
    pub features: Vec<Feature>,
}

#[derive(Clone, Debug, Default)]
pub struct Tile {
    pub layers: HashMap<String, Layer>,
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }
    fn done(&self) -> bool {
        self.at >= self.bytes.len()
    }
    fn varint(&mut self) -> Result<u64, String> {
        let mut value = 0u64;
        for shift in (0..64).step_by(7) {
            let byte = *self.bytes.get(self.at).ok_or("truncated varint")?;
            self.at += 1;
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err("varint too long".into())
    }
    fn key(&mut self) -> Result<(u64, u8), String> {
        let key = self.varint()?;
        Ok((key >> 3, (key & 7) as u8))
    }
    fn bytes(&mut self) -> Result<&'a [u8], String> {
        let len = self.varint()? as usize;
        let end = self.at.checked_add(len).filter(|&e| e <= self.bytes.len());
        let end = end.ok_or("truncated field")?;
        let slice = &self.bytes[self.at..end];
        self.at = end;
        Ok(slice)
    }
    fn fixed(&mut self, size: usize) -> Result<&'a [u8], String> {
        let end = self.at + size;
        let slice = self
            .bytes
            .get(self.at..end)
            .ok_or("truncated fixed field")?;
        self.at = end;
        Ok(slice)
    }
    fn skip(&mut self, wire: u8) -> Result<(), String> {
        match wire {
            0 => self.varint().map(drop),
            1 => self.fixed(8).map(drop),
            2 => self.bytes().map(drop),
            5 => self.fixed(4).map(drop),
            _ => Err(format!("unsupported wire type {wire}")),
        }
    }
    fn packed(&mut self) -> Result<Vec<u32>, String> {
        let mut inner = Reader::new(self.bytes()?);
        let mut out = Vec::new();
        while !inner.done() {
            out.push(inner.varint()? as u32);
        }
        Ok(out)
    }
}

/// Decode a whole tile. Gzip-compressed bodies are inflated first.
pub fn decode(bytes: &[u8]) -> Result<Tile, String> {
    let inflated;
    let bytes = if bytes.starts_with(&[0x1f, 0x8b]) {
        use std::io::Read;
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(bytes)
            .read_to_end(&mut out)
            .map_err(|e| format!("gzip: {e}"))?;
        inflated = out;
        &inflated[..]
    } else {
        bytes
    };
    let mut tile = Tile::default();
    let mut r = Reader::new(bytes);
    while !r.done() {
        match r.key()? {
            (3, 2) => {
                let (name, layer) = decode_layer(r.bytes()?)?;
                tile.layers.insert(name, layer);
            }
            (_, wire) => r.skip(wire)?,
        }
    }
    Ok(tile)
}

fn decode_layer(bytes: &[u8]) -> Result<(String, Layer), String> {
    let mut r = Reader::new(bytes);
    let (mut name, mut extent) = (String::new(), 4096.);
    let (mut keys, mut values, mut raw) = (Vec::new(), Vec::new(), Vec::new());
    while !r.done() {
        match r.key()? {
            (1, 2) => name = String::from_utf8_lossy(r.bytes()?).into_owned(),
            (2, 2) => raw.push(r.bytes()?),
            (3, 2) => keys.push(String::from_utf8_lossy(r.bytes()?).into_owned()),
            (4, 2) => values.push(decode_value(r.bytes()?)?),
            (5, 0) => extent = r.varint()? as f32,
            (_, wire) => r.skip(wire)?,
        }
    }
    let mut features = Vec::with_capacity(raw.len());
    for bytes in raw {
        if let Some(feature) = decode_feature(bytes, &keys, &values)? {
            features.push(feature);
        }
    }
    Ok((name, Layer { extent, features }))
}

fn decode_value(bytes: &[u8]) -> Result<Value, String> {
    let mut r = Reader::new(bytes);
    let mut value = Value::Str(String::new());
    while !r.done() {
        value = match r.key()? {
            (1, 2) => Value::Str(String::from_utf8_lossy(r.bytes()?).into_owned()),
            (2, 5) => Value::Num(f64::from(f32::from_le_bytes(
                r.fixed(4)?.try_into().unwrap(),
            ))),
            (3, 1) => Value::Num(f64::from_le_bytes(r.fixed(8)?.try_into().unwrap())),
            (4, 0) => Value::Num(r.varint()? as i64 as f64),
            (5, 0) => Value::Num(r.varint()? as f64),
            (6, 0) => {
                let v = r.varint()?;
                Value::Num(((v >> 1) as i64 ^ -((v & 1) as i64)) as f64)
            }
            (7, 0) => Value::Bool(r.varint()? != 0),
            (_, wire) => {
                r.skip(wire)?;
                continue;
            }
        };
    }
    Ok(value)
}

fn decode_feature(
    bytes: &[u8],
    keys: &[String],
    values: &[Value],
) -> Result<Option<Feature>, String> {
    let mut r = Reader::new(bytes);
    let (mut kind, mut tags, mut geometry) = (0, Vec::new(), Vec::new());
    while !r.done() {
        match r.key()? {
            (2, 2) => tags = r.packed()?,
            (3, 0) => kind = r.varint()?,
            (4, 2) => geometry = r.packed()?,
            (_, wire) => r.skip(wire)?,
        }
    }
    let kind = match kind {
        1 => Geometry::Point,
        2 => Geometry::Line,
        3 => Geometry::Polygon,
        _ => return Ok(None),
    };
    let mut properties = HashMap::with_capacity(tags.len() / 2);
    for pair in tags.chunks_exact(2) {
        if let (Some(k), Some(v)) = (keys.get(pair[0] as usize), values.get(pair[1] as usize)) {
            properties.insert(k.clone(), v.clone());
        }
    }
    Ok(Some(Feature {
        kind,
        parts: decode_geometry(&geometry, kind),
        properties,
    }))
}

/// Commands: MoveTo (1), LineTo (2), ClosePath (7), with zigzag deltas.
fn decode_geometry(commands: &[u32], kind: Geometry) -> Vec<Vec<[f32; 2]>> {
    let zigzag = |v: u32| ((v >> 1) as i32) ^ -((v & 1) as i32);
    let (mut parts, mut current): (Vec<Vec<[f32; 2]>>, Vec<[f32; 2]>) = (Vec::new(), Vec::new());
    let (mut x, mut y, mut i) = (0i32, 0i32, 0usize);
    while i < commands.len() {
        let (id, count) = (commands[i] & 7, commands[i] >> 3);
        i += 1;
        match id {
            1 | 2 => {
                for _ in 0..count {
                    let (Some(&dx), Some(&dy)) = (commands.get(i), commands.get(i + 1)) else {
                        return parts;
                    };
                    i += 2;
                    x += zigzag(dx);
                    y += zigzag(dy);
                    if id == 1 && kind != Geometry::Point && !current.is_empty() {
                        parts.push(std::mem::take(&mut current));
                    }
                    current.push([x as f32, y as f32]);
                    if kind == Geometry::Point {
                        parts.push(std::mem::take(&mut current));
                    }
                }
            }
            7 => {
                if let Some(&first) = current.first() {
                    current.push(first);
                }
            }
            _ => return parts,
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn geometry_commands_decode_points_lines_and_closed_rings() {
        // MoveTo(1) (2,2); LineTo(2) (+3,0) (0,+3); ClosePath.
        let ring = [9, 4, 4, 18, 6, 0, 0, 6, 15];
        let parts = decode_geometry(&ring, Geometry::Polygon);
        assert_eq!(parts, vec![vec![[2., 2.], [5., 2.], [5., 5.], [2., 2.]]]);
        // Two points from one MoveTo with count 2.
        let points = decode_geometry(&[17, 2, 2, 4, 4], Geometry::Point);
        assert_eq!(points, vec![vec![[1., 1.]], vec![[3., 3.]]]);
    }
}
