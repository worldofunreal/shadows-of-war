//! Versioned `map.bin` and `catalog.bin` formats (no JSON).

pub const MAP_MAGIC: &[u8; 4] = b"SOWM";
pub const MAP_VERSION: u16 = 1;
/// Transitional inline-geo layout (record between spawns and terrain);
/// still parsed, never written. Current files append the geo record AFTER
/// the terrain instead, staying version 1 so pre-geo parsers (deployed
/// servers, cached wasm clients) read them unchanged and ignore the tail.
pub const MAP_VERSION_INLINE_GEO: u16 = 2;

/// Geo record tags: byte introducing the record.
const GEO_TAG_NONE: u8 = 0;
const GEO_TAG_EQUIRECT: u8 = 1;
const MAP_ROSTER_TAG: u8 = 2;
const MAP_ROSTER_VERSION: u16 = 1;

pub const CATALOG_MAGIC: &[u8; 4] = b"SOWC";
/// v2 adds land/frequency. Roster summaries use a tagged trailing extension,
/// which v2 readers safely ignore; v3 inline rosters are accepted for transition.
pub const CATALOG_VERSION: u16 = 2;
const CATALOG_VERSION_INLINE_ROSTERS: u16 = 3;
const CATALOG_ROSTER_TAG: u8 = 1;
const CATALOG_ROSTER_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MapRosterRole {
    Nation,
    Tribe,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MapRosterEntry {
    pub entity_id: String,
    pub role: MapRosterRole,
    pub x: u32,
    pub y: u32,
    #[serde(default)]
    pub legacy_anchor: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MapRosterPreset {
    pub id: String,
    pub name: String,
    pub entries: Vec<MapRosterEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MapRosterMeta {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MapSpawn {
    pub name: String,
    pub flag: String,
    pub x: u32,
    pub y: u32,
}

/// Geographic bounding box of a real-world map, equirectangular projection.
/// Degrees stored as fixed-point micro-degrees (E6) for exact roundtrip and `Eq`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GeoBounds {
    pub min_lon_e6: i32,
    pub min_lat_e6: i32,
    pub max_lon_e6: i32,
    pub max_lat_e6: i32,
}

impl GeoBounds {
    pub fn from_degrees(min_lon: f64, min_lat: f64, max_lon: f64, max_lat: f64) -> Self {
        Self {
            min_lon_e6: (min_lon * 1e6).round() as i32,
            min_lat_e6: (min_lat * 1e6).round() as i32,
            max_lon_e6: (max_lon * 1e6).round() as i32,
            max_lat_e6: (max_lat * 1e6).round() as i32,
        }
    }

    pub fn min_lon(&self) -> f64 {
        self.min_lon_e6 as f64 / 1e6
    }
    pub fn min_lat(&self) -> f64 {
        self.min_lat_e6 as f64 / 1e6
    }
    pub fn max_lon(&self) -> f64 {
        self.max_lon_e6 as f64 / 1e6
    }
    pub fn max_lat(&self) -> f64 {
        self.max_lat_e6 as f64 / 1e6
    }

    /// Pacific-centered maps cross the antimeridian and store `max_lon > 180`;
    /// input longitudes stay in [-180, 180] and get shifted up into the box.
    fn normalize_lon(&self, lon: f64) -> f64 {
        if self.max_lon_e6 > 180_000_000 && lon < self.min_lon() {
            lon + 360.0
        } else {
            lon
        }
    }

    pub fn contains(&self, lat: f64, lon: f64) -> bool {
        let lon = self.normalize_lon(lon);
        lat >= self.min_lat()
            && lat <= self.max_lat()
            && lon >= self.min_lon()
            && lon <= self.max_lon()
    }

    /// lat/lon → tile coordinate. Equirectangular (linear lon→x, lat→y),
    /// mirroring the map generators' projection. Returns `None` outside the
    /// bounds or on a degenerate bbox. f64 basic ops only: bit-identical
    /// across native and wasm targets (lockstep-safe).
    pub fn project(&self, lat: f64, lon: f64, width: u32, height: u32) -> Option<(u32, u32)> {
        let lon_span = self.max_lon() - self.min_lon();
        let lat_span = self.max_lat() - self.min_lat();
        if lon_span <= 0.0 || lat_span <= 0.0 || !self.contains(lat, lon) {
            return None;
        }
        let lon = self.normalize_lon(lon);
        let x = ((lon - self.min_lon()) / lon_span * width as f64) as u32;
        let y = ((self.max_lat() - lat) / lat_span * height as f64) as u32;
        Some((
            x.min(width.saturating_sub(1)),
            y.min(height.saturating_sub(1)),
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapHeader {
    pub display_name: String,
    pub width: u32,
    pub height: u32,
    pub num_land_tiles: u32,
    pub spawn_count: usize,
    pub geo_bounds: Option<GeoBounds>,
    pub default_roster: Option<String>,
    pub roster_presets: Vec<MapRosterMeta>,
    pub header_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapFile {
    pub display_name: String,
    pub width: u32,
    pub height: u32,
    pub num_land_tiles: u32,
    pub spawns: Vec<MapSpawn>,
    pub geo_bounds: Option<GeoBounds>,
    pub default_roster: Option<String>,
    pub rosters: Vec<MapRosterPreset>,
    pub terrain: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapCatalogEntry {
    pub key: String,
    pub display_name: String,
    pub width: u32,
    pub height: u32,
    /// Number of land tiles — drives matchmaking's neutral AI population.
    pub num_land_tiles: u32,
    /// Weighted-rotation tickets (`multiplayer_frequency`); 0 = out of rotation.
    pub multiplayer_frequency: u32,
    pub default_roster: Option<String>,
    pub roster_presets: Vec<MapRosterMeta>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapCatalog {
    pub entries: Vec<MapCatalogEntry>,
}

#[derive(Debug)]
pub enum MapFileError {
    TooShort,
    BadMagic,
    UnsupportedVersion(u16),
    BadGeoTag(u8),
    BadMapTag(u8),
    BadCatalogTag(u8),
    InvalidCatalog(&'static str),
    InvalidRoster(&'static str),
    InvalidUtf8,
    TerrainLengthMismatch { expected: usize, got: usize },
}

impl std::fmt::Display for MapFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort => write!(f, "map file too short"),
            Self::BadMagic => write!(f, "invalid map magic (expected SOWM)"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported map version {v}"),
            Self::BadGeoTag(t) => write!(f, "unsupported geo record tag {t}"),
            Self::BadMapTag(t) => write!(f, "unsupported map metadata tag {t}"),
            Self::BadCatalogTag(t) => write!(f, "unsupported catalog metadata tag {t}"),
            Self::InvalidCatalog(message) => write!(f, "invalid map catalog: {message}"),
            Self::InvalidRoster(message) => write!(f, "invalid map roster: {message}"),
            Self::InvalidUtf8 => write!(f, "invalid utf-8 in map file"),
            Self::TerrainLengthMismatch { expected, got } => {
                write!(f, "terrain length mismatch: expected {expected}, got {got}")
            }
        }
    }
}

impl std::error::Error for MapFileError {}

fn read_u16(data: &[u8], off: &mut usize) -> Option<u16> {
    let bytes = data.get(*off..*off + 2)?;
    *off += 2;
    Some(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32(data: &[u8], off: &mut usize) -> Option<u32> {
    let bytes = data.get(*off..*off + 4)?;
    *off += 4;
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_i32(data: &[u8], off: &mut usize) -> Option<i32> {
    let bytes = data.get(*off..*off + 4)?;
    *off += 4;
    Some(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_u8(data: &[u8], off: &mut usize) -> Option<u8> {
    let b = *data.get(*off)?;
    *off += 1;
    Some(b)
}

/// Read the v2+ geo record (tag byte + optional bbox).
fn read_geo_record(data: &[u8], off: &mut usize) -> Result<Option<GeoBounds>, MapFileError> {
    let tag = read_u8(data, off).ok_or(MapFileError::TooShort)?;
    match tag {
        GEO_TAG_NONE => Ok(None),
        GEO_TAG_EQUIRECT => {
            let min_lon_e6 = read_i32(data, off).ok_or(MapFileError::TooShort)?;
            let min_lat_e6 = read_i32(data, off).ok_or(MapFileError::TooShort)?;
            let max_lon_e6 = read_i32(data, off).ok_or(MapFileError::TooShort)?;
            let max_lat_e6 = read_i32(data, off).ok_or(MapFileError::TooShort)?;
            Ok(Some(GeoBounds {
                min_lon_e6,
                min_lat_e6,
                max_lon_e6,
                max_lat_e6,
            }))
        }
        other => Err(MapFileError::BadGeoTag(other)),
    }
}

fn read_string(data: &[u8], off: &mut usize) -> Result<String, MapFileError> {
    let len = read_u16(data, off).ok_or(MapFileError::TooShort)? as usize;
    let slice = data.get(*off..*off + len).ok_or(MapFileError::TooShort)?;
    *off += len;
    std::str::from_utf8(slice)
        .map(|s| s.to_owned())
        .map_err(|_| MapFileError::InvalidUtf8)
}

fn write_u16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn write_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn write_i32(out: &mut Vec<u8>, v: i32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn write_string(out: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    debug_assert!(bytes.len() <= u16::MAX as usize);
    write_u16(out, bytes.len() as u16);
    out.extend_from_slice(bytes);
}

fn read_roster_record(
    data: &[u8],
    off: &mut usize,
    width: u32,
    height: u32,
    spawn_count: usize,
) -> Result<(Option<String>, Vec<MapRosterPreset>), MapFileError> {
    let tag = read_u8(data, off).ok_or(MapFileError::TooShort)?;
    if tag != MAP_ROSTER_TAG {
        return Err(MapFileError::BadMapTag(tag));
    }
    let len = read_u32(data, off).ok_or(MapFileError::TooShort)? as usize;
    let end = (*off).checked_add(len).ok_or(MapFileError::TooShort)?;
    let payload = data.get(*off..end).ok_or(MapFileError::TooShort)?;
    *off = end;
    let mut cursor = 0;
    let version = read_u16(payload, &mut cursor).ok_or(MapFileError::TooShort)?;
    if version != MAP_ROSTER_VERSION {
        return Err(MapFileError::InvalidRoster("unsupported roster version"));
    }
    let default_id = read_string(payload, &mut cursor)?;
    let count = read_u16(payload, &mut cursor).ok_or(MapFileError::TooShort)? as usize;
    let mut presets = Vec::with_capacity(count);
    let mut preset_ids = std::collections::HashSet::with_capacity(count);
    for _ in 0..count {
        let id = read_string(payload, &mut cursor)?;
        let name = read_string(payload, &mut cursor)?;
        if id.is_empty() || name.trim().is_empty() {
            return Err(MapFileError::InvalidRoster("empty preset ID or name"));
        }
        let entry_count = read_u16(payload, &mut cursor).ok_or(MapFileError::TooShort)? as usize;
        let mut entries = Vec::with_capacity(entry_count);
        let mut entity_ids = std::collections::HashSet::with_capacity(entry_count);
        let mut tiles = std::collections::HashSet::with_capacity(entry_count);
        let mut anchors = std::collections::HashSet::with_capacity(entry_count);
        for _ in 0..entry_count {
            let entity_id = read_string(payload, &mut cursor)?;
            let role = match read_u8(payload, &mut cursor).ok_or(MapFileError::TooShort)? {
                0 => MapRosterRole::Nation,
                1 => MapRosterRole::Tribe,
                _ => return Err(MapFileError::InvalidRoster("unknown gameplay role")),
            };
            let x = read_u32(payload, &mut cursor).ok_or(MapFileError::TooShort)?;
            let y = read_u32(payload, &mut cursor).ok_or(MapFileError::TooShort)?;
            let anchor = read_u16(payload, &mut cursor).ok_or(MapFileError::TooShort)?;
            if entity_id.is_empty() {
                return Err(MapFileError::InvalidRoster(
                    "empty preset or entity ID/name",
                ));
            }
            if x >= width || y >= height {
                return Err(MapFileError::InvalidRoster(
                    "entity position is outside the map",
                ));
            }
            if !entity_ids.insert(entity_id.clone()) || !tiles.insert((x, y)) {
                return Err(MapFileError::InvalidRoster(
                    "duplicate entity or tile in preset",
                ));
            }
            if anchor != u16::MAX && anchor as usize >= spawn_count {
                return Err(MapFileError::InvalidRoster(
                    "legacy anchor index is out of range",
                ));
            }
            if anchor != u16::MAX && !anchors.insert(anchor) {
                return Err(MapFileError::InvalidRoster(
                    "legacy anchor linked twice in preset",
                ));
            }
            entries.push(MapRosterEntry {
                entity_id,
                role,
                x,
                y,
                legacy_anchor: (anchor != u16::MAX).then_some(anchor),
            });
        }
        if !preset_ids.insert(id.clone()) {
            return Err(MapFileError::InvalidRoster("duplicate preset ID"));
        }
        presets.push(MapRosterPreset { id, name, entries });
    }
    if cursor != payload.len() {
        return Err(MapFileError::InvalidRoster(
            "unexpected bytes in roster record",
        ));
    }
    if presets.is_empty() || !presets.iter().any(|preset| preset.id == default_id) {
        return Err(MapFileError::InvalidRoster("default preset is missing"));
    }
    Ok((Some(default_id), presets))
}

fn parse_map_tail(
    data: &[u8],
    mut off: usize,
    version: u16,
    width: u32,
    height: u32,
    spawn_count: usize,
    initial_geo: Option<GeoBounds>,
) -> Result<(Option<GeoBounds>, Option<String>, Vec<MapRosterPreset>), MapFileError> {
    let mut geo_bounds = initial_geo;
    if version == MAP_VERSION && off < data.len() {
        geo_bounds = read_geo_record(data, &mut off)?;
    }
    let (default_roster, rosters) = if off < data.len() {
        read_roster_record(data, &mut off, width, height, spawn_count)?
    } else {
        (None, Vec::new())
    };
    if off != data.len() {
        return Err(MapFileError::BadMapTag(data[off]));
    }
    Ok((geo_bounds, default_roster, rosters))
}

fn write_roster_record(map: &MapFile, out: &mut Vec<u8>) {
    if map.rosters.is_empty() {
        return;
    }
    let mut payload = Vec::new();
    write_u16(&mut payload, MAP_ROSTER_VERSION);
    write_string(
        &mut payload,
        map.default_roster.as_deref().unwrap_or_default(),
    );
    write_u16(&mut payload, map.rosters.len() as u16);
    for preset in &map.rosters {
        write_string(&mut payload, &preset.id);
        write_string(&mut payload, &preset.name);
        write_u16(&mut payload, preset.entries.len() as u16);
        for entry in &preset.entries {
            write_string(&mut payload, &entry.entity_id);
            payload.push(match entry.role {
                MapRosterRole::Nation => 0,
                MapRosterRole::Tribe => 1,
            });
            write_u32(&mut payload, entry.x);
            write_u32(&mut payload, entry.y);
            write_u16(&mut payload, entry.legacy_anchor.unwrap_or(u16::MAX));
        }
    }
    out.push(MAP_ROSTER_TAG);
    write_u32(out, payload.len() as u32);
    out.extend_from_slice(&payload);
}

/// Validate map-authored roster data before writing it back to a map file.
/// `require_catalog_ids` is used by the editor save boundary; reading old maps
/// remains possible after an Atlas entry is removed so it can be repaired.
pub fn validate_rosters(map: &MapFile, require_catalog_ids: bool) -> Result<(), String> {
    if map.rosters.is_empty() {
        return if map.default_roster.is_none() {
            Ok(())
        } else {
            Err("default roster is set but this map has no presets".into())
        };
    }
    if map.rosters.len() > u16::MAX as usize {
        return Err("too many roster presets".into());
    }
    let mut presets = std::collections::HashSet::new();
    if !map
        .rosters
        .iter()
        .any(|preset| Some(&preset.id) == map.default_roster.as_ref())
    {
        return Err("default roster must name one of this map's presets".into());
    }
    for preset in &map.rosters {
        if !valid_roster_key(&preset.id)
            || preset.id.len() > u16::MAX as usize
            || preset.name.trim().is_empty()
            || preset.name.len() > u16::MAX as usize
            || preset.entries.len() > u16::MAX as usize
            || !presets.insert(&preset.id)
        {
            return Err(format!("invalid or duplicate roster preset: {}", preset.id));
        }
        let mut entities = std::collections::HashSet::new();
        let mut tiles = std::collections::HashSet::new();
        let mut anchors = std::collections::HashSet::new();
        for entry in &preset.entries {
            if !valid_roster_key(&entry.entity_id)
                || entry.entity_id.len() > u16::MAX as usize
                || entry.x >= map.width
                || entry.y >= map.height
                || !entities.insert(&entry.entity_id)
                || !tiles.insert((entry.x, entry.y))
            {
                return Err(format!(
                    "invalid or duplicate entry in roster {}",
                    preset.id
                ));
            }
            if let Some(anchor) = entry.legacy_anchor {
                if anchor as usize >= map.spawns.len() || !anchors.insert(anchor) {
                    return Err(format!(
                        "invalid or duplicate linked anchor in roster {}",
                        preset.id
                    ));
                }
            }
            if require_catalog_ids && sow_data::geo_entities::by_id(&entry.entity_id).is_none() {
                return Err(format!("unknown Atlas entity ID: {}", entry.entity_id));
            }
        }
    }
    Ok(())
}

/// Carry map-authored rosters across a map-source replacement without silently
/// retargeting linked legacy anchors. A changed map size or spawn table needs an
/// explicit edit in the map-roster editor before the replacement is written.
pub fn preserve_rosters(previous: &MapFile, replacement: &mut MapFile) -> Result<(), &'static str> {
    if previous.rosters.is_empty() {
        return Ok(());
    }
    if previous.width != replacement.width
        || previous.height != replacement.height
        || previous.spawns != replacement.spawns
    {
        return Err("map dimensions or spawn anchors changed; roster needs review in Map Rosters");
    }
    replacement.default_roster = previous.default_roster.clone();
    replacement.rosters = previous.rosters.clone();
    Ok(())
}

fn valid_roster_key(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some('a'..='z'))
        && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
}

/// Parse map header + spawn table; does not copy terrain.
pub fn parse_header(data: &[u8]) -> Result<MapHeader, MapFileError> {
    if data.len() < 20 {
        return Err(MapFileError::TooShort);
    }
    if data.get(0..4) != Some(MAP_MAGIC) {
        return Err(MapFileError::BadMagic);
    }
    let mut off = 4usize;
    let version = read_u16(data, &mut off).ok_or(MapFileError::TooShort)?;
    if version != MAP_VERSION && version != MAP_VERSION_INLINE_GEO {
        return Err(MapFileError::UnsupportedVersion(version));
    }
    off += 2; // reserved
    let width = read_u32(data, &mut off).ok_or(MapFileError::TooShort)?;
    let height = read_u32(data, &mut off).ok_or(MapFileError::TooShort)?;
    let num_land_tiles = read_u32(data, &mut off).ok_or(MapFileError::TooShort)?;
    let display_name = read_string(data, &mut off)?;
    let spawn_count = read_u16(data, &mut off).ok_or(MapFileError::TooShort)? as usize;
    for _ in 0..spawn_count {
        let _name = read_string(data, &mut off)?;
        let _flag = read_string(data, &mut off)?;
        let _ = read_u32(data, &mut off).ok_or(MapFileError::TooShort)?;
        let _ = read_u32(data, &mut off).ok_or(MapFileError::TooShort)?;
    }
    let mut geo_bounds = None;
    if version == MAP_VERSION_INLINE_GEO {
        // Transitional layout: record sits between spawns and terrain.
        geo_bounds = read_geo_record(data, &mut off)?;
    }
    let header_bytes = off;
    let mut default_roster = None;
    let mut roster_presets = Vec::new();
    let terrain_end = (width as usize)
        .checked_mul(height as usize)
        .and_then(|len| header_bytes.checked_add(len));
    if let Some(terrain_end) = terrain_end.filter(|end| data.len() >= *end) {
        let (parsed_geo, default, rosters) = parse_map_tail(
            data,
            terrain_end,
            version,
            width,
            height,
            spawn_count,
            geo_bounds,
        )?;
        geo_bounds = parsed_geo;
        default_roster = default;
        roster_presets = rosters
            .into_iter()
            .map(|preset| MapRosterMeta {
                id: preset.id,
                name: preset.name,
            })
            .collect();
    }
    Ok(MapHeader {
        display_name,
        width,
        height,
        num_land_tiles,
        spawn_count,
        geo_bounds,
        default_roster,
        roster_presets,
        header_bytes,
    })
}

/// Full map parse (header + spawns + terrain).
pub fn parse(data: &[u8]) -> Result<MapFile, MapFileError> {
    let header = parse_header(data)?;
    let terrain_len = (header.width as usize)
        .checked_mul(header.height as usize)
        .ok_or(MapFileError::TooShort)?;
    let terrain = data
        .get(header.header_bytes..header.header_bytes + terrain_len)
        .ok_or(MapFileError::TerrainLengthMismatch {
            expected: terrain_len,
            got: data.len().saturating_sub(header.header_bytes),
        })?
        .to_vec();
    if terrain.len() != terrain_len {
        return Err(MapFileError::TerrainLengthMismatch {
            expected: terrain_len,
            got: terrain.len(),
        });
    }

    let mut off = 4usize;
    let _version = read_u16(data, &mut off).ok_or(MapFileError::TooShort)?;
    off += 2;
    let _width = read_u32(data, &mut off).ok_or(MapFileError::TooShort)?;
    let _height = read_u32(data, &mut off).ok_or(MapFileError::TooShort)?;
    let num_land_tiles = read_u32(data, &mut off).ok_or(MapFileError::TooShort)?;
    let display_name = read_string(data, &mut off)?;
    let spawn_count = read_u16(data, &mut off).ok_or(MapFileError::TooShort)? as usize;
    let mut spawns = Vec::with_capacity(spawn_count);
    for _ in 0..spawn_count {
        let name = read_string(data, &mut off)?;
        let flag = read_string(data, &mut off)?;
        let x = read_u32(data, &mut off).ok_or(MapFileError::TooShort)?;
        let y = read_u32(data, &mut off).ok_or(MapFileError::TooShort)?;
        spawns.push(MapSpawn { name, flag, x, y });
    }

    Ok(MapFile {
        display_name,
        width: header.width,
        height: header.height,
        num_land_tiles,
        spawns,
        geo_bounds: header.geo_bounds,
        default_roster: header.default_roster,
        rosters: if header.roster_presets.is_empty() {
            Vec::new()
        } else {
            let terrain_end = header.header_bytes + terrain_len;
            let (_, _, rosters) = parse_map_tail(
                data,
                terrain_end,
                u16::from_le_bytes([data[4], data[5]]),
                header.width,
                header.height,
                header.spawn_count,
                header.geo_bounds,
            )?;
            rosters
        },
        terrain,
    })
}

pub fn encode(map: &MapFile) -> Vec<u8> {
    let terrain_len = (map.width as usize) * (map.height as usize);
    debug_assert_eq!(map.terrain.len(), terrain_len);

    // Always version 1: the optional geo record trails the terrain, where
    // pre-geo parsers never look. Unstamped maps stay byte-identical to the
    // original format.
    let mut out = Vec::with_capacity(32 + map.terrain.len());
    out.extend_from_slice(MAP_MAGIC);
    write_u16(&mut out, MAP_VERSION);
    write_u16(&mut out, 0);
    write_u32(&mut out, map.width);
    write_u32(&mut out, map.height);
    write_u32(&mut out, map.num_land_tiles);
    write_string(&mut out, &map.display_name);
    write_u16(&mut out, map.spawns.len() as u16);
    for s in &map.spawns {
        write_string(&mut out, &s.name);
        write_string(&mut out, &s.flag);
        write_u32(&mut out, s.x);
        write_u32(&mut out, s.y);
    }
    out.extend_from_slice(&map.terrain);
    if map.geo_bounds.is_some() || !map.rosters.is_empty() {
        if let Some(b) = map.geo_bounds {
            out.push(GEO_TAG_EQUIRECT);
            write_i32(&mut out, b.min_lon_e6);
            write_i32(&mut out, b.min_lat_e6);
            write_i32(&mut out, b.max_lon_e6);
            write_i32(&mut out, b.max_lat_e6);
        } else {
            // Old readers consume this known no-bounds record, then ignore the
            // roster extension that follows it.
            out.push(GEO_TAG_NONE);
        }
    }
    write_roster_record(map, &mut out);
    out
}

pub fn encode_catalog(catalog: &MapCatalog) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(CATALOG_MAGIC);
    write_u16(&mut out, CATALOG_VERSION);
    write_u16(&mut out, 0);
    write_u32(&mut out, catalog.entries.len() as u32);
    for e in &catalog.entries {
        write_string(&mut out, &e.key);
        write_string(&mut out, &e.display_name);
        write_u32(&mut out, e.width);
        write_u32(&mut out, e.height);
        write_u32(&mut out, e.num_land_tiles);
        write_u32(&mut out, e.multiplayer_frequency);
    }
    if catalog
        .entries
        .iter()
        .any(|entry| entry.default_roster.is_some() || !entry.roster_presets.is_empty())
    {
        let mut payload = Vec::new();
        write_u16(&mut payload, CATALOG_ROSTER_VERSION);
        write_u32(&mut payload, catalog.entries.len() as u32);
        for entry in &catalog.entries {
            write_string(&mut payload, &entry.key);
            write_string(
                &mut payload,
                entry.default_roster.as_deref().unwrap_or_default(),
            );
            write_u16(&mut payload, entry.roster_presets.len() as u16);
            for preset in &entry.roster_presets {
                write_string(&mut payload, &preset.id);
                write_string(&mut payload, &preset.name);
            }
        }
        out.push(CATALOG_ROSTER_TAG);
        write_u32(&mut out, payload.len() as u32);
        out.extend_from_slice(&payload);
    }
    out
}

pub fn parse_catalog(data: &[u8]) -> Result<MapCatalog, MapFileError> {
    if data.len() < 12 {
        return Err(MapFileError::TooShort);
    }
    if data.get(0..4) != Some(CATALOG_MAGIC) {
        return Err(MapFileError::BadMagic);
    }
    let mut off = 4usize;
    let version = read_u16(data, &mut off).ok_or(MapFileError::TooShort)?;
    if version > CATALOG_VERSION_INLINE_ROSTERS {
        return Err(MapFileError::UnsupportedVersion(version));
    }
    off += 2;
    let count = read_u32(data, &mut off).ok_or(MapFileError::TooShort)? as usize;
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let key = read_string(data, &mut off)?;
        let display_name = read_string(data, &mut off)?;
        let width = read_u32(data, &mut off).ok_or(MapFileError::TooShort)?;
        let height = read_u32(data, &mut off).ok_or(MapFileError::TooShort)?;
        let (num_land_tiles, multiplayer_frequency) = if version >= 2 {
            (
                read_u32(data, &mut off).ok_or(MapFileError::TooShort)?,
                read_u32(data, &mut off).ok_or(MapFileError::TooShort)?,
            )
        } else {
            (0, 1)
        };
        let (default_roster, roster_presets) = if version >= CATALOG_VERSION_INLINE_ROSTERS {
            let default = read_string(data, &mut off)?;
            let count = read_u16(data, &mut off).ok_or(MapFileError::TooShort)? as usize;
            let mut presets = Vec::with_capacity(count);
            for _ in 0..count {
                presets.push(MapRosterMeta {
                    id: read_string(data, &mut off)?,
                    name: read_string(data, &mut off)?,
                });
            }
            ((!default.is_empty()).then_some(default), presets)
        } else {
            (None, Vec::new())
        };
        entries.push(MapCatalogEntry {
            key,
            display_name,
            width,
            height,
            num_land_tiles,
            multiplayer_frequency,
            default_roster,
            roster_presets,
        });
    }
    if off < data.len() {
        if version >= CATALOG_VERSION_INLINE_ROSTERS {
            return Err(MapFileError::InvalidCatalog(
                "unexpected bytes after inline roster metadata",
            ));
        }
        let tag = read_u8(data, &mut off).ok_or(MapFileError::TooShort)?;
        if tag != CATALOG_ROSTER_TAG {
            return Err(MapFileError::BadCatalogTag(tag));
        }
        let len = read_u32(data, &mut off).ok_or(MapFileError::TooShort)? as usize;
        let end = off.checked_add(len).ok_or(MapFileError::TooShort)?;
        let payload = data.get(off..end).ok_or(MapFileError::TooShort)?;
        off = end;
        let mut cursor = 0usize;
        if read_u16(payload, &mut cursor).ok_or(MapFileError::TooShort)? != CATALOG_ROSTER_VERSION {
            return Err(MapFileError::InvalidCatalog(
                "unsupported roster summary version",
            ));
        }
        let metadata_count = read_u32(payload, &mut cursor).ok_or(MapFileError::TooShort)? as usize;
        if metadata_count != entries.len() {
            return Err(MapFileError::InvalidCatalog(
                "roster summary map count does not match catalog",
            ));
        }
        for entry in &mut entries {
            let key = read_string(payload, &mut cursor)?;
            if key != entry.key {
                return Err(MapFileError::InvalidCatalog(
                    "roster summary map order does not match catalog",
                ));
            }
            let default = read_string(payload, &mut cursor)?;
            let preset_count =
                read_u16(payload, &mut cursor).ok_or(MapFileError::TooShort)? as usize;
            let mut presets = Vec::with_capacity(preset_count);
            for _ in 0..preset_count {
                presets.push(MapRosterMeta {
                    id: read_string(payload, &mut cursor)?,
                    name: read_string(payload, &mut cursor)?,
                });
            }
            if !default.is_empty() && !presets.iter().any(|preset| preset.id == default) {
                return Err(MapFileError::InvalidCatalog(
                    "default roster summary is not listed",
                ));
            }
            entry.default_roster = (!default.is_empty()).then_some(default);
            entry.roster_presets = presets;
        }
        if cursor != payload.len() || off != data.len() {
            return Err(MapFileError::InvalidCatalog(
                "unexpected bytes in roster summary",
            ));
        }
    }
    Ok(MapCatalog { entries })
}

/// Build catalog from map folder keys and parsed headers.
/// Frequency comes from the folder's `info.toml` (written by sow-tools).
pub fn catalog_from_headers(
    items: impl IntoIterator<Item = (String, MapHeader, u32)>,
) -> MapCatalog {
    let mut entries: Vec<MapCatalogEntry> = items
        .into_iter()
        .map(|(key, h, frequency)| MapCatalogEntry {
            key,
            display_name: h.display_name,
            width: h.width,
            height: h.height,
            num_land_tiles: h.num_land_tiles,
            multiplayer_frequency: frequency,
            default_roster: h.default_roster,
            roster_presets: h.roster_presets,
        })
        .collect();
    entries.sort_by_key(|a| a.display_name.to_lowercase());
    MapCatalog { entries }
}

/// Parse `frequency` from an `info.toml` blob (`multiplayer_frequency`
/// ticket count for the weighted rotation; 0 = out of rotation). Pure, no I/O —
/// callers read the file. Missing/unparsable → default 1.
pub fn parse_frequency_toml(blob: &str) -> u32 {
    for line in blob.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("frequency") {
            let rest = rest.trim_start_matches(['=', ' ', '\t']);
            if let Ok(v) = rest.trim().parse::<u32>() {
                return v;
            }
        }
    }
    1
}

/// Decompress a `map.bin.br` payload (no-op if already decompressed SOWM).
pub fn decompress_map_payload(bytes: &[u8]) -> Result<Vec<u8>, MapFileError> {
    if bytes.len() >= 4 && &bytes[0..4] == MAP_MAGIC {
        return Ok(bytes.to_vec());
    }
    let mut out = Vec::new();
    let mut decoder = brotli::Decompressor::new(bytes, 4096);
    std::io::Read::read_to_end(&mut decoder, &mut out).map_err(|_| MapFileError::TooShort)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_map(geo_bounds: Option<GeoBounds>) -> MapFile {
        MapFile {
            display_name: "North America".to_string(),
            width: 4,
            height: 4,
            num_land_tiles: 10,
            spawns: vec![MapSpawn {
                name: "Rome".to_string(),
                flag: "it".to_string(),
                x: 1,
                y: 2,
            }],
            geo_bounds,
            default_roster: None,
            rosters: Vec::new(),
            terrain: vec![0u8; 16],
        }
    }

    #[test]
    fn roundtrip_map_file() {
        let map = sample_map(None);
        let bytes = encode(&map);
        let parsed = parse(&bytes).unwrap();
        assert_eq!(parsed.display_name, "North America");
        assert_eq!(parsed.spawns.len(), 1);
        assert_eq!(parsed.terrain.len(), 16);
        assert_eq!(parsed.geo_bounds, None);
    }

    #[test]
    fn map_roster_roundtrips_without_rewriting_legacy_map_fields() {
        let mut map = sample_map(Some(GeoBounds::from_degrees(-20.0, -10.0, 30.0, 40.0)));
        map.default_roster = Some("historical".into());
        map.rosters = vec![MapRosterPreset {
            id: "historical".into(),
            name: "Historical".into(),
            entries: vec![MapRosterEntry {
                entity_id: "iceland".into(),
                role: MapRosterRole::Nation,
                x: 2,
                y: 3,
                legacy_anchor: Some(0),
            }],
        }];

        validate_rosters(&map, true).unwrap();
        let bytes = encode(&map);
        let parsed = parse(&bytes).unwrap();
        assert_eq!(parsed.display_name, map.display_name);
        assert_eq!(parsed.spawns, map.spawns);
        assert_eq!(parsed.terrain, map.terrain);
        assert_eq!(parsed.geo_bounds, map.geo_bounds);
        assert_eq!(parsed.default_roster, map.default_roster);
        assert_eq!(parsed.rosters, map.rosters);
        let header = parse_header(&bytes).unwrap();
        assert_eq!(header.default_roster.as_deref(), Some("historical"));
        assert_eq!(header.roster_presets[0].id, "historical");

        let mut compressed = Vec::new();
        {
            let mut writer = brotli::CompressorWriter::new(&mut compressed, 4096, 5, 22);
            std::io::Write::write_all(&mut writer, &bytes).unwrap();
        }
        assert_eq!(decompress_map_payload(&compressed).unwrap(), bytes);
    }

    #[test]
    fn roster_survives_source_refresh_only_when_anchors_are_unchanged() {
        let mut previous = sample_map(None);
        previous.default_roster = Some("historical".into());
        previous.rosters = vec![MapRosterPreset {
            id: "historical".into(),
            name: "Historical".into(),
            entries: vec![MapRosterEntry {
                entity_id: "iceland".into(),
                role: MapRosterRole::Nation,
                x: 2,
                y: 3,
                legacy_anchor: Some(0),
            }],
        }];

        let mut replacement = sample_map(None);
        preserve_rosters(&previous, &mut replacement).unwrap();
        assert_eq!(replacement.rosters, previous.rosters);
        assert_eq!(replacement.default_roster, previous.default_roster);
        assert_eq!(replacement.spawns, previous.spawns);

        replacement.spawns[0].name = "Different anchor".into();
        replacement.rosters.clear();
        replacement.default_roster = None;
        assert!(preserve_rosters(&previous, &mut replacement).is_err());
        assert!(replacement.rosters.is_empty());
        assert!(replacement.default_roster.is_none());
    }

    #[test]
    fn no_roster_map_still_ends_after_terrain_and_keeps_legacy_anchors() {
        let map = sample_map(None);
        let bytes = encode(&map);
        let header = parse_header(&bytes).unwrap();
        assert_eq!(bytes.len(), header.header_bytes + map.terrain.len());
        assert_eq!(parse(&bytes).unwrap().spawns, map.spawns);
    }

    #[test]
    fn no_bounds_encodes_as_v1() {
        let bytes = encode(&sample_map(None));
        assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), 1);
    }

    #[test]
    fn roundtrip_with_trailing_bounds() {
        let bounds = GeoBounds::from_degrees(-30.5, 10.25, 60.0, 72.125);
        let map = sample_map(Some(bounds));
        let bytes = encode(&map);
        // Stays version 1: pre-geo parsers must accept stamped maps.
        assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), 1);
        let parsed = parse(&bytes).unwrap();
        assert_eq!(parsed.geo_bounds, Some(bounds));
        assert_eq!(parsed.terrain.len(), 16);
        assert_eq!(parsed.spawns, map.spawns);
        let header = parse_header(&bytes).unwrap();
        assert_eq!(header.geo_bounds, Some(bounds));
        // Terrain sits at header_bytes; the geo record trails it.
        let terrain_end = header.header_bytes + map.terrain.len();
        assert_eq!(&bytes[header.header_bytes..terrain_end], &map.terrain[..]);
        assert_eq!(bytes.len(), terrain_end + 17);
    }

    /// A stamped file must parse under the ORIGINAL v1 rules (what deployed
    /// servers and cached wasm clients run): version==1, terrain sliced by
    /// exact length from the spawn-table end, trailing bytes ignored.
    #[test]
    fn stamped_file_readable_by_pre_geo_parser() {
        let map = sample_map(Some(GeoBounds::from_degrees(0.0, 0.0, 10.0, 10.0)));
        let bytes = encode(&map);
        // Header prefix only (what a pre-geo parse_header consumed) must be
        // byte-identical to the unstamped encoding's prefix.
        let unstamped = encode(&sample_map(None));
        let header = parse_header(&unstamped).unwrap();
        assert_eq!(
            bytes[..header.header_bytes],
            unstamped[..header.header_bytes]
        );
        // Old parse: exact-length terrain slice succeeds.
        let terrain = bytes
            .get(header.header_bytes..header.header_bytes + map.terrain.len())
            .unwrap();
        assert_eq!(terrain, &map.terrain[..]);
    }

    /// Transitional inline layout (version 2, record between spawns and
    /// terrain) is still parsed for files stamped before the format moved
    /// the record behind the terrain.
    #[test]
    fn parses_transitional_inline_geo_layout() {
        let bounds = GeoBounds::from_degrees(-5.0, 40.0, 25.0, 60.0);
        let map = sample_map(Some(bounds));
        let mut bytes = Vec::new();
        bytes.extend_from_slice(MAP_MAGIC);
        write_u16(&mut bytes, MAP_VERSION_INLINE_GEO);
        write_u16(&mut bytes, 0);
        write_u32(&mut bytes, map.width);
        write_u32(&mut bytes, map.height);
        write_u32(&mut bytes, map.num_land_tiles);
        write_string(&mut bytes, &map.display_name);
        write_u16(&mut bytes, map.spawns.len() as u16);
        for s in &map.spawns {
            write_string(&mut bytes, &s.name);
            write_string(&mut bytes, &s.flag);
            write_u32(&mut bytes, s.x);
            write_u32(&mut bytes, s.y);
        }
        bytes.push(1); // GEO_TAG_EQUIRECT inline
        write_i32(&mut bytes, bounds.min_lon_e6);
        write_i32(&mut bytes, bounds.min_lat_e6);
        write_i32(&mut bytes, bounds.max_lon_e6);
        write_i32(&mut bytes, bounds.max_lat_e6);
        bytes.extend_from_slice(&map.terrain);
        let parsed = parse(&bytes).unwrap();
        assert_eq!(parsed.geo_bounds, Some(bounds));
        assert_eq!(parsed.terrain, map.terrain);
    }

    #[test]
    fn truncated_geo_record_is_too_short() {
        let bounds = GeoBounds::from_degrees(0.0, 0.0, 10.0, 10.0);
        let bytes = encode(&sample_map(Some(bounds)));
        // Cut inside the trailing geo record (after terrain).
        let cut = bytes.len() - 8;
        assert!(matches!(parse(&bytes[..cut]), Err(MapFileError::TooShort)));
    }

    #[test]
    fn future_version_rejected() {
        let mut bytes = encode(&sample_map(None));
        bytes[4..6].copy_from_slice(&3u16.to_le_bytes());
        assert!(matches!(
            parse(&bytes),
            Err(MapFileError::UnsupportedVersion(3))
        ));
    }

    #[test]
    fn bad_geo_tag_rejected() {
        let bounds = GeoBounds::from_degrees(0.0, 0.0, 10.0, 10.0);
        let map = sample_map(Some(bounds));
        let mut bytes = encode(&map);
        let tag_off = bytes.len() - 17;
        assert_eq!(bytes[tag_off], 1);
        bytes[tag_off] = 7;
        assert!(matches!(parse(&bytes), Err(MapFileError::BadGeoTag(7))));
    }

    #[test]
    fn project_maps_bounds_to_tiles() {
        // World-style full bbox on a 1000x800 grid.
        let b = GeoBounds::from_degrees(-180.0, -90.0, 180.0, 90.0);
        assert_eq!(b.project(90.0, -180.0, 1000, 800), Some((0, 0)));
        assert_eq!(b.project(-90.0, 180.0, 1000, 800), Some((999, 799)));
        // Equator/meridian center.
        assert_eq!(b.project(0.0, 0.0, 1000, 800), Some((500, 400)));
        // Matches poi_extractor formula: x=(lon-min_lon)*scale, y=(max_lat-lat)*scale.
        let e = GeoBounds::from_degrees(-10.0, 35.0, 40.0, 70.0);
        let (w, h) = (500u32, 350u32); // 10 px/degree
        let (x, y) = e.project(48.85, 2.35, w, h).unwrap(); // Paris
        assert_eq!(x, ((2.35 - -10.0) * 10.0) as u32);
        assert_eq!(y, ((70.0 - 48.85) * 10.0) as u32);
        // Outside → None.
        assert_eq!(e.project(40.7, -74.0, w, h), None); // New York not in Europe
    }

    #[test]
    fn project_handles_antimeridian_wrap() {
        // Pacific box from lon 90 to 210 (= -150): Hawaii at -155 is inside.
        let b = GeoBounds::from_degrees(90.0, -50.0, 210.0, 30.0);
        assert!(b.contains(21.3, -157.86)); // Honolulu
        assert!(b.contains(-36.85, 174.76)); // Auckland
        assert!(!b.contains(48.85, 2.35)); // Paris
        let (w, h) = (1200u32, 800u32);
        let (x_fiji, _) = b.project(-17.8, 178.0, w, h).unwrap();
        let (x_hawaii, _) = b.project(21.3, -157.86, w, h).unwrap();
        assert!(x_hawaii > x_fiji, "Hawaii must land east of Fiji");
    }

    #[test]
    fn roundtrip_catalog() {
        let cat = MapCatalog {
            entries: vec![MapCatalogEntry {
                key: "world".to_string(),
                display_name: "North America".to_string(),
                width: 100,
                height: 100,
                num_land_tiles: 5000,
                multiplayer_frequency: 20,
                default_roster: Some("historical".to_string()),
                roster_presets: vec![MapRosterMeta {
                    id: "historical".to_string(),
                    name: "Historical".to_string(),
                }],
            }],
        };
        let bytes = encode_catalog(&cat);
        assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), CATALOG_VERSION);
        let parsed = parse_catalog(&bytes).unwrap();
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].key, "world");
        assert_eq!(parsed.entries[0].num_land_tiles, 5000);
        assert_eq!(parsed.entries[0].multiplayer_frequency, 20);
        assert_eq!(
            parsed.entries[0].default_roster.as_deref(),
            Some("historical")
        );
        assert_eq!(parsed.entries[0].roster_presets[0].name, "Historical");
    }

    #[test]
    fn roster_catalog_keeps_the_legacy_v2_entry_prefix() {
        let catalog = MapCatalog {
            entries: vec![MapCatalogEntry {
                key: "world".into(),
                display_name: "World".into(),
                width: 4,
                height: 4,
                num_land_tiles: 12,
                multiplayer_frequency: 1,
                default_roster: Some("historical".into()),
                roster_presets: vec![MapRosterMeta {
                    id: "historical".into(),
                    name: "Historical".into(),
                }],
            }],
        };
        let bytes = encode_catalog(&catalog);
        let mut offset = 4;
        assert_eq!(read_u16(&bytes, &mut offset), Some(2));
        offset += 2;
        let count = read_u32(&bytes, &mut offset).unwrap();
        assert_eq!(count, 1);
        let _ = read_string(&bytes, &mut offset).unwrap();
        let _ = read_string(&bytes, &mut offset).unwrap();
        offset += 16; // width, height, land count, rotation frequency
        assert_eq!(bytes[offset], CATALOG_ROSTER_TAG);

        // The deployed v2 reader stops after these entries and ignores trailing bytes.
        let legacy = parse_catalog(&bytes[..offset]).unwrap();
        assert_eq!(legacy.entries[0].default_roster, None);
        assert!(legacy.entries[0].roster_presets.is_empty());
        assert_eq!(
            parse_catalog(&bytes).unwrap().entries[0]
                .default_roster
                .as_deref(),
            Some("historical")
        );
    }

    #[test]
    fn reads_transitional_inline_v3_roster_catalog() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(CATALOG_MAGIC);
        write_u16(&mut bytes, CATALOG_VERSION_INLINE_ROSTERS);
        write_u16(&mut bytes, 0);
        write_u32(&mut bytes, 1);
        write_string(&mut bytes, "world");
        write_string(&mut bytes, "World");
        write_u32(&mut bytes, 4);
        write_u32(&mut bytes, 4);
        write_u32(&mut bytes, 12);
        write_u32(&mut bytes, 1);
        write_string(&mut bytes, "historical");
        write_u16(&mut bytes, 1);
        write_string(&mut bytes, "historical");
        write_string(&mut bytes, "Historical");

        let parsed = parse_catalog(&bytes).unwrap();
        assert_eq!(
            parsed.entries[0].default_roster.as_deref(),
            Some("historical")
        );
        assert_eq!(parsed.entries[0].roster_presets[0].name, "Historical");
    }

    #[test]
    fn parse_frequency_toml_ok() {
        assert_eq!(parse_frequency_toml("frequency = 7\n"), 7);
        assert_eq!(parse_frequency_toml("frequency=0\n"), 0);
        assert_eq!(parse_frequency_toml(""), 1);
        assert_eq!(parse_frequency_toml("frequency = 20\n# comment\n"), 20);
    }
}
