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

/// A playable hero's ability: one generic effect plus the situation it needs.
/// The enum is closed on purpose: each kind is programmed once in sow-core, and
/// the hero ability tests run every hero through the real game formulas, so a
/// hero cannot ship an ability that does nothing. Hero names live only in the
/// catalog text. Two heroes must never share a situation (see `situation`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Ability {
    /// Strength per troop of attacks against `against` rises by `percent`.
    AttackStrength { against: Target, percent: u32 },
    /// Attacks against `against` conquer `percent` faster.
    ConquestSpeed { against: Target, percent: u32 },
    /// Defending armies are harder to conquer for attackers of `against`.
    DefenseStrength { against: Target, percent: u32 },
    /// `source` pays `percent` more of `resource`.
    ResourceBonus {
        source: Source,
        resource: Resource,
        percent: u32,
    },
    /// Tower fights: a defender's towers make attackers lose `percent` more
    /// troops; an attacker's armies lose `percent` fewer at enemy towers.
    TowerLoss { side: Side, percent: u32 },
    /// City tiles: a defender's city needs `percent` more effort to capture;
    /// an attacker captures city tiles `percent` faster.
    CityCapture { side: Side, percent: u32 },
    /// City tiles resist attacks from `against` by `percent` more capture effort.
    CityDefense { against: Target, percent: u32 },
    /// Attackers lose `percent` fewer troops when `when` holds.
    AttackerLoss { when: LossWhen, percent: u32 },
    /// Cities raise the troop cap more.
    CityTroopCapacity { percent: u32 },
    /// Towers defend a wider area.
    TowerRange { percent: u32 },
    /// Campaign alliances with `against` cost this much less gold.
    AllianceCost { against: Target, percent: u32 },
    /// Building a warship costs this much less gold.
    WarshipCost { percent: u32 },
    /// Enemy armies of `against` retreating from this hero's land keep fewer
    /// troops: the retreat penalty is multiplied by `1 + percent/100`.
    EnemyRetreatPenalty { against: Target, percent: u32 },
    /// Establish one free port after gaining an enemy coast with a legal site.
    CoastalFounding,
    /// A living ally's newly completed city pays both rulers once.
    AlliedCityGrant { gold: u32 },
    /// Completing a city upgrade raises a nearby ready tower by one level.
    CityTowerUpgrade,
    /// A newly captured enemy city pays a fixed purse shared with living allies.
    SharedCitySpoils { gold: u32 },
    /// Losing a city mobilizes reserves once per city, within the troop cap.
    CityEvacuation { troops: u32 },
    /// A retreat after at least three seconds opens a thirty-second return window.
    ReturnCampaign { percent: u32 },
    /// Receive a share of the strongest larger ally's ordinary gold production.
    AllyPatronage { percent: u32 },
    /// At peace, convert otherwise wasted troop production into gold.
    SurplusTreasury { troops_per_gold: u32 },
    /// Factory levels near a ready city pay additional gold.
    GuildNetwork { gold_per_second: u32 },
    /// A lethal naval shell takes an enemy warship as a damaged prize.
    WarshipPrize { percent: u32 },
}

/// Who an attack, conquest or retreat is aimed at. `Larger`, `Smaller`, `Richer`,
/// `Poorer`, `Kin` and `Foreign` compare that player with the hero's own player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    /// Any target.
    Anyone,
    /// Neutral land (no owner).
    Neutral,
    /// Land of a player, not neutral land.
    OtherPlayer,
    /// Land of a human player.
    Human,
    /// Land of a tribe (a bot owner).
    Tribe,
    /// Land of a nation (a nation owner).
    Nation,
    /// Land whose owner has an active traitor status.
    ActiveTraitor,
    /// Land whose owner has more troops than the army.
    Outnumbering,
    /// Land whose owner has fewer troops than the army.
    Outnumbered,
    /// Land whose owner has more tiles than the hero's player.
    Larger,
    /// Land whose owner has fewer tiles than the hero's player.
    Smaller,
    /// Land whose owner has more gold than the hero's player.
    Richer,
    /// Land whose owner has less gold than the hero's player.
    Poorer,
    /// Land of another player shown as the same people or realm (`GeoEntity::entity`)
    /// as the hero's player. A catalog bot is shown as its own catalog people.
    Kin,
    /// Land of a player, not neutral land, shown as a different people or realm
    /// (`GeoEntity::entity`) than the hero's player.
    Foreign,
}

impl Target {
    /// Every target, for checking which ones describe a player.
    pub const ALL: [Target; 15] = [
        Target::Anyone,
        Target::Neutral,
        Target::OtherPlayer,
        Target::Human,
        Target::Tribe,
        Target::Nation,
        Target::ActiveTraitor,
        Target::Outnumbering,
        Target::Outnumbered,
        Target::Larger,
        Target::Smaller,
        Target::Richer,
        Target::Poorer,
        Target::Kin,
        Target::Foreign,
    ];
}

/// Where a `ResourceBonus` comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    District(District),
    Territory,
    Alliance,
    Elimination,
    /// Trade ships.
    Trade,
}

/// Which side of a fight a tower or city modifier works on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Defender,
    Attacker,
}

/// When an attacker loses fewer troops, for `AttackerLoss`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LossWhen {
    /// Conquering mountain tiles.
    Mountain,
    /// Conquering highland tiles.
    Highland,
    /// Retreating from enemy land.
    Retreat,
    /// Attacking an enemy that took one of this hero's cities, while that
    /// enemy is still inside the recapture window.
    RecaptureWindow,
    /// Conquering city tiles.
    City,
    /// Conquering land on the coast: a land tile next to water.
    Coast,
    /// Troops that landed from ships, while they take enemy land.
    Landing,
    /// Conquering land that the target describes (`{"against": "richer"}`).
    Against(Target),
}

impl Ability {
    pub fn percent(self) -> u32 {
        match self {
            Ability::AttackStrength { percent, .. }
            | Ability::ConquestSpeed { percent, .. }
            | Ability::DefenseStrength { percent, .. }
            | Ability::ResourceBonus { percent, .. }
            | Ability::TowerLoss { percent, .. }
            | Ability::CityCapture { percent, .. }
            | Ability::CityDefense { percent, .. }
            | Ability::AttackerLoss { percent, .. }
            | Ability::CityTroopCapacity { percent }
            | Ability::TowerRange { percent }
            | Ability::AllianceCost { percent, .. }
            | Ability::WarshipCost { percent }
            | Ability::ReturnCampaign { percent }
            | Ability::AllyPatronage { percent }
            | Ability::WarshipPrize { percent }
            | Ability::EnemyRetreatPenalty { percent, .. } => percent,
            Ability::CoastalFounding
            | Ability::AlliedCityGrant { .. }
            | Ability::CityTowerUpgrade
            | Ability::SharedCitySpoils { .. }
            | Ability::CityEvacuation { .. }
            | Ability::SurplusTreasury { .. }
            | Ability::GuildNetwork { .. } => 0,
        }
    }

    /// The same situation with its magnitude removed; fixed actions stay unchanged.
    /// Used for uniqueness and to select control cases in effect tests.
    pub fn situation(self) -> Ability {
        match self {
            Ability::AttackStrength { against, .. } => Ability::AttackStrength {
                against,
                percent: 0,
            },
            Ability::ConquestSpeed { against, .. } => Ability::ConquestSpeed {
                against,
                percent: 0,
            },
            Ability::DefenseStrength { against, .. } => Ability::DefenseStrength {
                against,
                percent: 0,
            },
            Ability::ResourceBonus {
                source, resource, ..
            } => Ability::ResourceBonus {
                source,
                resource,
                percent: 0,
            },
            Ability::TowerLoss { side, .. } => Ability::TowerLoss { side, percent: 0 },
            Ability::CityCapture { side, .. } => Ability::CityCapture { side, percent: 0 },
            Ability::CityDefense { against, .. } => Ability::CityDefense {
                against,
                percent: 0,
            },
            Ability::AttackerLoss { when, .. } => Ability::AttackerLoss { when, percent: 0 },
            Ability::CityTroopCapacity { .. } => Ability::CityTroopCapacity { percent: 0 },
            Ability::TowerRange { .. } => Ability::TowerRange { percent: 0 },
            Ability::AllianceCost { against, .. } => Ability::AllianceCost {
                against,
                percent: 0,
            },
            Ability::WarshipCost { .. } => Ability::WarshipCost { percent: 0 },
            Ability::EnemyRetreatPenalty { against, .. } => Ability::EnemyRetreatPenalty {
                against,
                percent: 0,
            },
            Ability::ReturnCampaign { .. } => Ability::ReturnCampaign { percent: 0 },
            Ability::AllyPatronage { .. } => Ability::AllyPatronage { percent: 0 },
            Ability::WarshipPrize { .. } => Ability::WarshipPrize { percent: 0 },
            Ability::AlliedCityGrant { .. } => Ability::AlliedCityGrant { gold: 0 },
            Ability::SharedCitySpoils { .. } => Ability::SharedCitySpoils { gold: 0 },
            Ability::CityEvacuation { .. } => Ability::CityEvacuation { troops: 0 },
            Ability::SurplusTreasury { .. } => Ability::SurplusTreasury { troops_per_gold: 0 },
            Ability::GuildNetwork { .. } => Ability::GuildNetwork { gold_per_second: 0 },
            Ability::CoastalFounding | Ability::CityTowerUpgrade => self,
        }
    }

    /// The visual family of this ability: the colour its effect is shown in.
    pub fn cue(self) -> Cue {
        match self {
            Ability::AttackStrength { .. } | Ability::DefenseStrength { .. } => Cue::Strength,
            Ability::ConquestSpeed { .. } => Cue::Conquest,
            Ability::ResourceBonus { .. } => Cue::Income,
            Ability::TowerLoss { .. } | Ability::TowerRange { .. } => Cue::Tower,
            Ability::CityCapture { .. }
            | Ability::CityDefense { .. }
            | Ability::CityTroopCapacity { .. } => Cue::City,
            Ability::AttackerLoss { .. } => Cue::Losses,
            Ability::AllianceCost { .. } => Cue::Alliance,
            Ability::WarshipCost { .. } => Cue::Naval,
            Ability::EnemyRetreatPenalty { .. } => Cue::Retreat,
            Ability::CoastalFounding | Ability::WarshipPrize { .. } => Cue::Naval,
            Ability::AlliedCityGrant { .. }
            | Ability::SharedCitySpoils { .. }
            | Ability::AllyPatronage { .. } => Cue::Alliance,
            Ability::CityTowerUpgrade => Cue::Tower,
            Ability::CityEvacuation { .. } => Cue::Retreat,
            Ability::ReturnCampaign { .. } => Cue::Strength,
            Ability::SurplusTreasury { .. } | Ability::GuildNetwork { .. } => Cue::Income,
        }
    }
}

/// The visual family of an ability. Every family has its own colour on the map, so
/// a hero's effect is recognisable from its colour before the notice is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cue {
    Strength,
    Conquest,
    Income,
    Tower,
    City,
    Losses,
    Alliance,
    Retreat,
    Naval,
}

impl Cue {
    /// Every family, so each one can be checked for its own colour.
    pub const ALL: [Cue; 9] = [
        Cue::Strength,
        Cue::Conquest,
        Cue::Income,
        Cue::Tower,
        Cue::City,
        Cue::Losses,
        Cue::Alliance,
        Cue::Retreat,
        Cue::Naval,
    ];
}

/// Fields only playable heroes have.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hero {
    /// Short name, saved as the key of each player's hero XP. Renamed heroes
    /// must retain a resolvable legacy ID so stored progress can migrate.
    pub name: String,
    /// IDs accepted from saves created before this hero was renamed.
    #[serde(default)]
    pub legacy_ids: Vec<String>,
    /// Art filenames; defaults to the catalog id.
    #[serde(default)]
    pub art_slug: Option<String>,
    pub ability: Ability,
    /// Additional tower-range bonus that can coexist with the hero's main ability.
    #[serde(default)]
    pub tower_range_bonus_percent: u32,
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
            // Tribe bots are named for their people; playable tribal leaders
            // keep their personal name and their tribe as the civilization.
            if entity.role == Role::Tribe && entity.hero.is_none() {
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

    /// A colour family no hero uses would be a colour that never shows on the map.
    #[test]
    fn every_cue_family_is_used_by_some_hero() {
        for cue in Cue::ALL {
            assert!(
                all()
                    .filter_map(|entity| entity.hero.as_ref())
                    .any(|hero| hero.ability.cue() == cue),
                "no playable hero shows the {cue:?} cue"
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
