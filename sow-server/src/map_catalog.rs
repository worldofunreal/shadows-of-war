use sow_core::map_file::{self, MapCatalog, MapCatalogEntry, MapHeader, MapRosterMeta};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

// ponytail: each reload leaks one catalog (a few KB); reloads are rare admin actions,
// so every caller keeps its `&'static` slice. Upgrade to `Arc` snapshots if reloads get frequent.
static CATALOG: RwLock<Option<(&'static [MapCatalogEntry], &'static [u8])>> = RwLock::new(None);

fn slug(name: &str) -> String {
    sow_core::maps::map_key(name)
}

/// Header and rotation weight for every readable map folder under `root`.
fn scan_items(root: &Path) -> Vec<(String, MapHeader, u32)> {
    let mut items = Vec::new();
    let Ok(read_dir) = std::fs::read_dir(root) else {
        return items;
    };
    for entry in read_dir.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let map_dir = entry.path();
        let key = entry.file_name().to_string_lossy().to_string();
        if key.starts_with('.') {
            continue;
        }
        // Try map.bin first (raw), fall back to map.bin.br (brotli-compressed, what deploy syncs).
        let bytes = if let Ok(data) = std::fs::read(map_dir.join("map.bin")) {
            data
        } else if let Ok(br) = std::fs::read(map_dir.join("map.bin.br")) {
            map_file::decompress_map_payload(&br).unwrap_or(br)
        } else {
            continue;
        };
        let Ok(header) = map_file::parse_header(&bytes) else {
            log::warn!("Skipping {key}: invalid map.bin header");
            continue;
        };
        let frequency = read_frequency(&map_dir);
        items.push((slug(&key), header, frequency));
    }
    items
}

pub fn scan_maps_root(root: &Path) -> MapCatalog {
    map_file::catalog_from_headers(scan_items(root))
}

/// Built-in maps first, then uploaded maps from `SOW_MAPS_UPLOAD_ROOT`. An uploaded
/// key that already exists in the built-in library is skipped, never overwritten.
fn scan_all(maps_root: &Path) -> MapCatalog {
    let mut roots = vec![maps_root.to_path_buf()];
    roots.extend(upload_root());
    let mut seen = HashSet::new();
    let mut items = Vec::new();
    for root in roots {
        for (key, header, frequency) in scan_items(&root) {
            if seen.insert(key.clone()) {
                items.push((key, header, frequency));
            } else {
                log::warn!("Skipping {key} from {}: key already in catalog", root.display());
            }
        }
    }
    map_file::catalog_from_headers(items)
}

/// Read the weighted-rotation ticket count for a map folder from its
/// `info.toml` (`frequency = N`; 0 = out of rotation). Missing → default 1.
fn read_frequency(map_dir: &Path) -> u32 {
    let Ok(blob) = std::fs::read_to_string(map_dir.join("info.toml")) else {
        return 1;
    };
    map_file::parse_frequency_toml(&blob)
}

pub fn init(maps_root: &Path) {
    if let Err(e) = publish(maps_root) {
        log::error!("Map catalog: {e}");
    }
}

/// Re-scan the map roots and publish the new catalog without a restart.
/// Returns the number of maps now listed.
pub fn reload() -> Result<usize, String> {
    publish(&maps_root())
}

fn publish(maps_root: &Path) -> Result<usize, String> {
    let catalog = scan_all(maps_root);
    let bytes = map_file::encode_catalog(&catalog);
    let count = catalog.entries.len();
    let entries: &'static [MapCatalogEntry] = Box::leak(catalog.entries.into_boxed_slice());
    let bytes_static: &'static [u8] = Box::leak(bytes.clone().into_boxed_slice());
    if let Ok(mut slot) = CATALOG.write() {
        *slot = Some((entries, bytes_static));
    }
    let catalog_path = std::env::var("SOW_MAPS_CATALOG_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| maps_root.join("catalog.bin"));
    std::fs::write(&catalog_path, &bytes)
        .map_err(|e| format!("failed to write {}: {e}", catalog_path.display()))?;
    log::info!("Map catalog: {count} entries → {}", catalog_path.display());
    Ok(count)
}

pub fn entries() -> &'static [MapCatalogEntry] {
    CATALOG
        .read()
        .ok()
        .and_then(|slot| *slot)
        .map(|(entries, _)| entries)
        .unwrap_or(&[])
}

pub fn catalog_bytes() -> &'static [u8] {
    CATALOG
        .read()
        .ok()
        .and_then(|slot| *slot)
        .map(|(_, bytes)| bytes)
        .unwrap_or(&[])
}

pub fn lookup(name: &str) -> Option<MapCatalogEntry> {
    sow_core::maps::catalog_lookup(entries(), name).cloned()
}

/// Folder where uploaded maps live, from `SOW_MAPS_UPLOAD_ROOT`. Outside the release, so deploys keep it.
pub fn upload_root() -> Option<PathBuf> {
    std::env::var_os("SOW_MAPS_UPLOAD_ROOT").map(PathBuf::from)
}

pub fn maps_root() -> PathBuf {
    std::env::var("SOW_MAPS_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(sow_core::maps::SERVER_MAPS_ROOT))
}

#[derive(serde::Serialize, Debug, Clone)]
pub struct MapCatalogJsonEntry {
    pub key: String,
    pub display_name: String,
    pub width: u32,
    pub height: u32,
    pub num_land_tiles: u32,
    pub multiplayer_frequency: u32,
    pub default_roster: Option<String>,
    pub roster_presets: Vec<MapRosterMeta>,
    pub thumbnail: String,
}

pub fn catalog_json() -> Vec<MapCatalogJsonEntry> {
    entries()
        .iter()
        .map(|entry| MapCatalogJsonEntry {
            key: entry.key.clone(),
            display_name: entry.display_name.clone(),
            width: entry.width,
            height: entry.height,
            num_land_tiles: entry.num_land_tiles,
            multiplayer_frequency: entry.multiplayer_frequency,
            default_roster: entry.default_roster.clone(),
            roster_presets: entry.roster_presets.clone(),
            thumbnail: format!("/maps/{}/thumbnail.webp", entry.key),
        })
        .collect()
}
