use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// Which bot list a catalog character belongs to: a nation (led by its leader)
/// or a tribe (named after its people). Playable heroes keep their origin here;
/// the `hero` block, not this role, makes them playable and keeps them out of `bots(..)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Nation,
    Tribe,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gender {
    Female,
    Male,
}

/// A building that produces income.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum District {
    City,
    Factory,
    Port,
    Farm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resource {
    Gold,
    Troops,
}

/// A playable hero's ability. The list is closed on purpose: every kind is
/// programmed once in sow-core, and the hero ability test exercises each
/// hero through the real game formulas, so a hero cannot ship an ability
/// that does nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Ability {
    /// Armies fight stronger (fewer losses, faster conquest).
    CombatStrength { percent: u32 },
    /// Attacks capture territory faster.
    ExpansionSpeed { percent: u32 },
    /// One district type produces more of one resource.
    DistrictIncome {
        district: District,
        resource: Resource,
        percent: u32,
    },
    /// Cities raise the troop cap more.
    CityTroopCapacity { percent: u32 },
    /// Bunkers defend a wider area.
    BunkerRange { percent: u32 },
    /// Bunkers make attackers lose more troops.
    BunkerDefense { percent: u32 },
    /// More gold for eliminating a player.
    EliminationGold { percent: u32 },
    /// Territory produces more gold.
    TerritoryGold { percent: u32 },
    /// Troops fight stronger when attacking another player (not neutral land).
    EnemyTroopStrength { percent: u32 },
    /// Enemies need more effort to capture this hero's city tiles.
    CityDefense { percent: u32 },
    /// Attacks on enemy city tiles capture faster.
    CityAssault { percent: u32 },
    /// Campaign alliances cost this much less gold.
    AllianceCost { percent: u32 },
    /// Every active alliance pays gold, `percent` more than the base rate.
    AllianceGold { percent: u32 },
    /// Attacks on a traitor's territory deal `percent` more damage to its defenders.
    DamageToTraitors { percent: u32 },
}

impl Ability {
    pub fn percent(self) -> u32 {
        match self {
            Ability::CombatStrength { percent }
            | Ability::ExpansionSpeed { percent }
            | Ability::DistrictIncome { percent, .. }
            | Ability::CityTroopCapacity { percent }
            | Ability::BunkerRange { percent }
            | Ability::BunkerDefense { percent }
            | Ability::EliminationGold { percent }
            | Ability::TerritoryGold { percent }
            | Ability::EnemyTroopStrength { percent }
            | Ability::CityDefense { percent }
            | Ability::CityAssault { percent }
            | Ability::AllianceCost { percent }
            | Ability::AllianceGold { percent }
            | Ability::DamageToTraitors { percent } => percent,
        }
    }
}

/// Fields only playable heroes have.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hero {
    /// Short name. Saved as the key of each player's hero XP: never rename.
    pub name: String,
    pub ability: Ability,
    /// NFTROPOLY/WOU-ID card token, only for heroes registered there.
    #[serde(default)]
    pub card: Option<String>,
    /// Always playable without buying it.
    #[serde(default)]
    pub free: bool,
}

/// One character of the shared catalog (`assets/geo_entities.json`): every
/// nation and tribe bot, playable heroes included.
/// `lat`/`lon` is the character's home; bots spawn spread over all land and
/// each spawn takes the free character whose home is closest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeoEntity {
    pub id: String,
    /// Shown in game: the leader's name for a nation, the tribe's name for a tribe.
    pub name: String,
    /// The people or realm the character leads.
    #[serde(default)]
    pub entity: String,
    pub role: Role,
    pub region: Region,
    #[serde(default)]
    pub lat: Option<f32>,
    #[serde(default)]
    pub lon: Option<f32>,
    #[serde(default)]
    pub gender: Option<Gender>,
    #[serde(default)]
    pub wikipedia: Option<String>,
    /// Name of the portrait file in `assets/art/avatars/*/` (see assets/art/README.md).
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub territory_color: Option<String>,
    /// Present only on playable heroes.
    #[serde(default)]
    pub hero: Option<Hero>,
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
        assert_eq!(catalog.version, 2, "unsupported entity catalog version");
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

/// Nation or tribe bots in catalog order (the order is stable across clients).
/// Playable heroes are never bots, whatever their origin.
pub fn bots(role: Role) -> impl Iterator<Item = &'static GeoEntity> {
    all().filter(move |entity| entity.role == role && entity.hero.is_none())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_entries_are_valid_and_stable() {
        let mut ids = std::collections::HashSet::new();
        let mut bot_names = std::collections::HashSet::new();
        let mut hero_names = std::collections::HashSet::new();
        for entity in all() {
            assert!(
                !entity.id.is_empty()
                    && entity.id.bytes().all(|byte| byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || byte == b'_'),
                "invalid ID {}",
                entity.id
            );
            assert!(ids.insert(entity.id.as_str()), "duplicate ID {}", entity.id);
            // A playable hero can intentionally share its historical name
            // with the bot representation of the same character.
            let names = if entity.hero.is_some() {
                &mut hero_names
            } else {
                &mut bot_names
            };
            assert!(
                names.insert(entity.name.to_lowercase()),
                "duplicate {} name {}",
                if entity.hero.is_some() { "hero" } else { "bot" },
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
            // Every bot needs a home (spawn assignment), a portrait folder,
            // a gender, a real region and a name of its own.
            assert!(
                entity.lat.is_some(),
                "{} has no home coordinates",
                entity.id
            );
            assert!(entity.avatar.is_some(), "{} has no avatar name", entity.id);
            assert!(entity.gender.is_some(), "{} has no gender", entity.id);
            assert_ne!(entity.region, Region::Unassigned, "{}", entity.id);
            assert!(
                !entity.entity.is_empty(),
                "{} has no people/realm",
                entity.id
            );
            if entity.role == Role::Tribe {
                assert_eq!(
                    entity.name, entity.entity,
                    "{} must be named after its people",
                    entity.id
                );
            }
        }
        let mut portraits = std::collections::HashSet::new();
        for character in all() {
            assert!(
                portraits.insert(character.avatar.as_deref()),
                "{} shares an avatar with another bot",
                character.id
            );
        }
    }

    #[test]
    fn playable_heroes_are_never_bots() {
        for hero in all().filter(|entity| entity.hero.is_some()) {
            assert!(
                !bots(Role::Nation)
                    .chain(bots(Role::Tribe))
                    .any(|bot| bot.id == hero.id),
                "{} is a playable hero and must not be a bot",
                hero.id
            );
        }
    }

    #[test]
    fn atlas_territory_color_accepts_hex_and_keeps_missing_colors_unset() {
        assert_eq!(
            parse_territory_color("#ff8000"),
            Some([1.0, 128.0 / 255.0, 0.0])
        );
        assert_eq!(parse_territory_color("#12xz00"), None);
        assert_eq!(parse_territory_color("#fff"), None);
    }
}
