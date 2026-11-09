//! A small decoder for Mapbox Vector Tiles (the open MVT 2.1 format that
//! Mapbox, OpenMapTiles, OpenFreeMap and others serve). Protobuf is decoded by
//! hand: the format uses only a handful of fields.
use std::collections::HashMap;

// Tiles cross a network boundary on desktop workers and on the browser's UI
// thread. Bound both decompression and the owned representation, not just HTTP.
pub(super) const MAX_TILE_BYTES: usize = 16 * 1024 * 1024;
const MAX_DECODED_BYTES: usize = 64 * 1024 * 1024;
const MAX_FEATURES: usize = 100_000;
const MAX_POINTS: usize = 1_000_000;
#[derive(Default)]
struct Budget {
    bytes: usize,
    features: usize,
    points: usize,
}
impl Budget {
    fn charge(&mut self, bytes: usize) -> Result<(), String> {
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .filter(|&n| n <= MAX_DECODED_BYTES)
            .ok_or("tile decoded allocation budget exceeded")?;
        Ok(())
    }
}

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
            if shift == 63 && byte > 1 {
                return Err("varint exceeds u64".into());
            }
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
        let len =
            usize::try_from(self.varint()?).map_err(|_| "field length exceeds platform range")?;
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
    fn packed(&mut self, limit: usize) -> Result<Vec<u32>, String> {
        let mut inner = Reader::new(self.bytes()?);
        let mut out = Vec::new();
        while !inner.done() {
            if out.len() >= limit {
                return Err("too many packed values".into());
            }
            out.push(u32::try_from(inner.varint()?).map_err(|_| "packed value exceeds u32")?);
        }
        Ok(out)
    }
}

/// Decode a whole tile. Gzip-compressed bodies are inflated first.
pub fn decode(bytes: &[u8]) -> Result<Tile, String> {
    if bytes.len() > MAX_TILE_BYTES {
        return Err("tile input exceeds 16 MiB limit".into());
    }
    let inflated;
    let bytes = if bytes.starts_with(&[0x1f, 0x8b]) {
        use std::io::Read;
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(bytes)
            .take((MAX_TILE_BYTES + 1) as u64)
            .read_to_end(&mut out)
            .map_err(|e| format!("gzip: {e}"))?;
        if out.len() > MAX_TILE_BYTES {
            return Err("expanded tile exceeds 16 MiB limit".into());
        }
        inflated = out;
        &inflated[..]
    } else {
        bytes
    };
    let mut tile = Tile::default();
    let mut budget = Budget::default();
    let mut layer_count = 0;
    let mut r = Reader::new(bytes);
    while !r.done() {
        match r.key()? {
            (3, 2) => {
                layer_count += 1;
                if layer_count > 128 {
                    return Err("too many tile layers".into());
                }
                let (name, layer) = decode_layer(r.bytes()?, &mut budget)?;
                tile.layers.insert(name, layer);
            }
            (_, wire) => r.skip(wire)?,
        }
    }
    Ok(tile)
}

fn decode_layer(bytes: &[u8], budget: &mut Budget) -> Result<(String, Layer), String> {
    let mut r = Reader::new(bytes);
    let (mut name, mut extent) = (String::new(), 4096.);
    let (mut keys, mut values, mut raw) = (Vec::new(), Vec::new(), Vec::new());
    while !r.done() {
        match r.key()? {
            (1, 2) => name = String::from_utf8_lossy(r.bytes()?).into_owned(),
            (2, 2) => {
                budget.features += 1;
                if budget.features > MAX_FEATURES {
                    return Err("too many tile features".into());
                }
                budget.charge(std::mem::size_of::<Feature>())?;
                raw.push(r.bytes()?);
            }
            (3, 2) => {
                let key = String::from_utf8_lossy(r.bytes()?).into_owned();
                budget.charge(key.len() + std::mem::size_of::<String>())?;
                keys.push(key);
            }
            (4, 2) => {
                let value = decode_value(r.bytes()?)?;
                budget.charge(
                    std::mem::size_of::<Value>()
                        + match &value {
                            Value::Str(s) => s.len(),
                            _ => 0,
                        },
                )?;
                values.push(value);
            }
            (5, 0) => {
                let value = u32::try_from(r.varint()?).map_err(|_| "tile extent exceeds u32")?;
                if value == 0 {
                    return Err("zero tile extent".into());
                }
                extent = value as f32;
            }
            (_, wire) => r.skip(wire)?,
        }
    }
    let mut features = Vec::with_capacity(raw.len());
    for bytes in raw {
        if let Some(feature) = decode_feature(bytes, &keys, &values, budget)? {
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
    budget: &mut Budget,
) -> Result<Option<Feature>, String> {
    let mut r = Reader::new(bytes);
    let (mut kind, mut tags, mut geometry) = (0, Vec::new(), Vec::new());
    while !r.done() {
        match r.key()? {
            (2, 2) => tags = r.packed(512)?,
            (3, 0) => kind = r.varint()?,
            (4, 2) => geometry = r.packed(MAX_POINTS * 3)?,
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
    for &[k, v] in tags.as_chunks::<2>().0 {
        if let (Some(k), Some(v)) = (keys.get(k as usize), values.get(v as usize)) {
            budget.charge(
                k.len()
                    + std::mem::size_of::<(String, Value)>()
                    + match v {
                        Value::Str(s) => s.len(),
                        _ => 0,
                    },
            )?;
            properties.insert(k.clone(), v.clone());
        }
    }
    Ok(Some(Feature {
        kind,
        parts: decode_geometry(&geometry, kind, budget)?,
        properties,
    }))
}

/// Commands: MoveTo (1), LineTo (2), ClosePath (7), with zigzag deltas.
fn decode_geometry(
    commands: &[u32],
    kind: Geometry,
    budget: &mut Budget,
) -> Result<Vec<Vec<[f32; 2]>>, String> {
    let zigzag = |v: u32| ((v >> 1) as i32) ^ -((v & 1) as i32);
    let (mut parts, mut current): (Vec<Vec<[f32; 2]>>, Vec<[f32; 2]>) = (Vec::new(), Vec::new());
    let (mut x, mut y, mut i) = (0i32, 0i32, 0usize);
    while i < commands.len() {
        let (id, count) = (commands[i] & 7, commands[i] >> 3);
        i += 1;
        if count == 0 {
            return Err("zero geometry command count".into());
        }
        match id {
            1 | 2 => {
                if (id == 2 && current.is_empty())
                    || (id == 1 && kind != Geometry::Point && count != 1)
                {
                    return Err("invalid geometry command sequence".into());
                }
                if count as usize > (commands.len() - i) / 2 {
                    return Err("truncated geometry command".into());
                }
                for _ in 0..count {
                    let (Some(&dx), Some(&dy)) = (commands.get(i), commands.get(i + 1)) else {
                        return Err("truncated geometry command".into());
                    };
                    i += 2;
                    x = x.checked_add(zigzag(dx)).ok_or("geometry x overflow")?;
                    y = y.checked_add(zigzag(dy)).ok_or("geometry y overflow")?;
                    budget.points += 1;
                    if budget.points > MAX_POINTS {
                        return Err("too many tile points".into());
                    }
                    budget.charge(std::mem::size_of::<[f32; 2]>())?;
                    if current.is_empty() {
                        budget.charge(std::mem::size_of::<Vec<[f32; 2]>>())?;
                    }
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
                if count != 1 || kind != Geometry::Polygon || current.len() < 3 {
                    return Err("invalid ClosePath command".into());
                }
                if let Some(&first) = current.first() {
                    budget.charge(std::mem::size_of::<[f32; 2]>())?;
                    current.push(first);
                }
            }
            _ => return Err("unknown geometry command".into()),
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    Ok(parts)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn geometry_commands_decode_points_lines_and_closed_rings() {
        // MoveTo(1) (2,2); LineTo(2) (+3,0) (0,+3); ClosePath.
        let ring = [9, 4, 4, 18, 6, 0, 0, 6, 15];
        let parts = decode_geometry(&ring, Geometry::Polygon, &mut Budget::default()).unwrap();
        assert_eq!(parts, vec![vec![[2., 2.], [5., 2.], [5., 5.], [2., 2.]]]);
        // Two points from one MoveTo with count 2.
        let points =
            decode_geometry(&[17, 2, 2, 4, 4], Geometry::Point, &mut Budget::default()).unwrap();
        assert_eq!(points, vec![vec![[1., 1.]], vec![[3., 3.]]]);
    }
}

#[cfg(test)]
mod adversarial_tests {
    use super::*;
    fn varint(mut value: u64, out: &mut Vec<u8>) {
        while value >= 128 {
            out.push(value as u8 | 128);
            value >>= 7;
        }
        out.push(value as u8);
    }
    fn field(tag: u8, data: &[u8], out: &mut Vec<u8>) {
        out.push(tag);
        varint(data.len() as u64, out);
        out.extend_from_slice(data);
    }
    #[test]
    fn network_geometry_coordinate_overflow_is_an_error_not_a_worker_panic() {
        let mut geometry = vec![];
        for value in [9, 0xffff_fffe, 0, 10, 2, 0] {
            varint(value, &mut geometry);
        }
        let mut feature = vec![0x18, 2];
        field(0x22, &geometry, &mut feature);
        let mut layer = vec![];
        field(0x0a, b"roads", &mut layer);
        field(0x12, &feature, &mut layer);
        let mut tile = vec![];
        field(0x1a, &layer, &mut tile);
        assert!(decode(&tile).is_err());
    }
    #[test]
    fn network_gzip_tiles_have_a_decompressed_size_limit() {
        use std::io::Write;
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(&vec![0; 16 * 1024 * 1024 + 2]).unwrap();
        let bytes = encoder.finish().unwrap();
        assert!(
            decode(&bytes).is_err(),
            "a tiny compressed tile must not expand without a cap"
        );
    }
}

#[cfg(test)]
mod limit_tests {
    use super::*;
    #[test]
    fn malformed_varints_and_uint32_fields_never_truncate() {
        let mut overflow = vec![0xff; 9];
        overflow.push(2);
        assert!(Reader::new(&overflow).varint().is_err());
        assert!(Reader::new(&[5, 0xff, 0xff, 0xff, 0xff, 0x1f])
            .packed(8)
            .is_err());
        assert!(decode(&[0x1a, 2, 0x28, 0]).is_err(), "zero extent");
    }
    #[test]
    fn malformed_commands_and_cumulative_quotas_fail_boundedly() {
        for commands in [&[0xffff_fff9][..], &[0][..], &[15, 15][..], &[11, 0, 0][..]] {
            assert!(decode_geometry(commands, Geometry::Line, &mut Budget::default()).is_err());
        }
        let mut points = Budget {
            points: MAX_POINTS,
            ..Budget::default()
        };
        assert!(decode_geometry(&[9, 0, 0], Geometry::Point, &mut points).is_err());
        let mut bytes = Budget {
            bytes: MAX_DECODED_BYTES - 1,
            ..Budget::default()
        };
        assert!(decode_geometry(&[9, 0, 0], Geometry::Point, &mut bytes).is_err());
        let empty_layers = [0x1a, 0].repeat(129);
        assert!(
            decode(&empty_layers).is_err(),
            "duplicate names must not bypass layer quota"
        );
    }
}
