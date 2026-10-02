use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Tribe,
    CityState,
    Kingdom,
    Empire,
    Country,
    StateRegion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Era {
    Unspecified,
    Ancient,
    Classical,
    Medieval,
    EarlyModern,
    Modern,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Region {
    Unassigned,
    Europe,
    Africa,
    Asia,
    Americas,
    Oceania,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeoEntity {
    pub id: String,
    pub name: String,
    pub kind: EntityKind,
    pub era: Era,
    pub region: Region,
    #[serde(default)]
    pub lat: Option<f32>,
    #[serde(default)]
    pub lon: Option<f32>,
    #[serde(default)]
    pub flag: String,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub territory_color: Option<String>,
    #[serde(default)]
    pub maps: Vec<String>,
    #[serde(default)]
    pub fallback_nation_order: Option<u32>,
    #[serde(default)]
    pub fallback_tribe_order: Option<u32>,
}

impl GeoEntity {
    pub fn territory_color_rgb(&self) -> Option<[f32; 3]> {
        parse_territory_color(self.territory_color.as_deref()?)
    }
}

fn parse_territory_color(color: &str) -> Option<[f32; 3]> {
    let hex = color.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let channel = |start| {
        u8::from_str_radix(hex.get(start..start + 2)?, 16)
            .ok()
            .map(|value| value as f32 / 255.0)
    };
    Some([channel(0)?, channel(2)?, channel(4)?])
}

#[derive(Debug, Deserialize)]
struct CatalogFile {
    version: u32,
    entities: Vec<GeoEntity>,
}

static CATALOG: OnceLock<CatalogFile> = OnceLock::new();

fn catalog() -> &'static CatalogFile {
    CATALOG.get_or_init(|| {
        let catalog: CatalogFile =
            serde_json::from_str(include_str!("../../../assets/geo_entities.json"))
                .expect("invalid assets/geo_entities.json");
        assert_eq!(catalog.version, 1, "unsupported entity catalog version");
        catalog
    })
}

/// Read the shared JSON once; all gameplay consumers use this stable ordering.
pub fn all() -> impl Iterator<Item = &'static GeoEntity> {
    catalog().entities.iter()
}

pub fn by_id(id: &str) -> Option<&'static GeoEntity> {
    all().find(|entity| entity.id == id)
}

pub fn by_name(name: &str) -> Option<&'static GeoEntity> {
    all().find(|entity| entity.name.eq_ignore_ascii_case(name))
}

pub fn flag_for_name(name: &str) -> Option<&'static str> {
    by_name(name)
        .map(|entity| entity.flag.as_str())
        .filter(|flag| !flag.is_empty())
}

pub fn fallback_nations() -> Vec<&'static GeoEntity> {
    ordered_pool(|entity| {
        if entity.kind == EntityKind::StateRegion {
            None
        } else {
            entity.fallback_nation_order
        }
    })
}

pub fn fallback_tribes() -> Vec<&'static GeoEntity> {
    ordered_pool(|entity| entity.fallback_tribe_order)
}

fn ordered_pool(order: impl Fn(&GeoEntity) -> Option<u32>) -> Vec<&'static GeoEntity> {
    let mut entries: Vec<_> = all()
        .filter_map(|entity| order(entity).map(|position| (position, entity)))
        .collect();
    entries.sort_unstable_by_key(|(position, _)| *position);
    entries.into_iter().map(|(_, entity)| entity).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_entries_are_valid_and_stable() {
        let mut ids = std::collections::HashSet::new();
        let mut names = std::collections::HashSet::new();
        let entities: Vec<_> = all().collect();
        assert!(entities.len() >= 1840, "catalog entity list shrank");

        for entity in &entities {
            assert!(!entity.id.is_empty());
            assert!(
                entity
                    .id
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
            );
            assert!(ids.insert(entity.id.as_str()), "duplicate ID {}", entity.id);
            assert!(
                names.insert(entity.name.to_lowercase()),
                "duplicate name {}",
                entity.name
            );
            assert_eq!(entity.lat.is_some(), entity.lon.is_some(), "{}", entity.id);
            if let (Some(lat), Some(lon)) = (entity.lat, entity.lon) {
                assert!((-90.0..=90.0).contains(&lat), "{} latitude", entity.id);
                assert!((-180.0..=180.0).contains(&lon), "{} longitude", entity.id);
            }
            if let Some(avatar) = &entity.avatar {
                assert!(
                    !avatar.is_empty()
                        && avatar.bytes().all(|byte| byte.is_ascii_lowercase()
                            || byte.is_ascii_digit()
                            || byte == b'_'),
                    "invalid avatar slug {}",
                    avatar
                );
            }
        }

        let nations = fallback_nations();
        let tribes = fallback_tribes();
        assert!(nations.len() >= 229);
        assert!(tribes.len() >= 670);
        assert!(
            nations
                .iter()
                .all(|entity| entity.kind != EntityKind::StateRegion)
        );
        assert!(
            nations
                .iter()
                .enumerate()
                .all(|(i, e)| e.fallback_nation_order == Some(i as u32))
        );
        assert!(
            tribes
                .iter()
                .enumerate()
                .all(|(i, e)| e.fallback_tribe_order == Some(i as u32))
        );
    }

    #[test]
    fn atlas_territory_color_accepts_hex_and_keeps_missing_colors_unset() {
        assert_eq!(parse_territory_color("#ff8000"), Some([1.0, 128.0 / 255.0, 0.0]));
        assert_eq!(parse_territory_color("#12xz00"), None);
        assert_eq!(parse_territory_color("#fff"), None);
    }

    #[test]
    fn every_campaign_faction_is_in_the_catalog() {
        let campaign_dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets/campaign");
        for entry in std::fs::read_dir(campaign_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if !path.extension().is_some_and(|ext| ext == "json")
                || path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .ends_with(".triggers.json")
            {
                continue;
            }
            let value: serde_json::Value =
                serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
            for faction in value["factions"].as_array().unwrap() {
                let name = faction["name"].as_str().unwrap();
                assert!(by_name(name).is_some(), "campaign entity missing: {name}");
            }
        }
    }
}
