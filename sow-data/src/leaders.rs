//! Playable heroes. A hero is a catalog character (`assets/geo_entities.json`)
//! with a `hero` entry: adding one there adds it to the game, the store and
//! the menu, and `hero_ability_tests` proves its ability works.
use crate::geo_entities::{self, Ability, GeoEntity};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::sync::OnceLock;

/// A playable hero, numbered by its position among the catalog's heroes.
/// The first hero (Caesar) is the default.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Leader(u16);

struct HeroEntry {
    character: &'static GeoEntity,
    /// Saved in profiles and messages: `sun_tzu` → `SunTzu`.
    wire_id: String,
}

fn heroes() -> &'static [HeroEntry] {
    static HEROES: OnceLock<Vec<HeroEntry>> = OnceLock::new();
    HEROES.get_or_init(|| {
        let heroes: Vec<HeroEntry> = geo_entities::all()
            .filter(|character| character.hero.is_some())
            .map(|character| HeroEntry {
                character,
                wire_id: character
                    .id
                    .split('_')
                    .map(|word| {
                        let mut letters = word.chars();
                        letters.next().map_or_else(String::new, |first| {
                            first.to_ascii_uppercase().to_string() + letters.as_str()
                        })
                    })
                    .collect(),
            })
            .collect();
        assert!(
            !heroes.is_empty() && heroes.len() <= usize::from(u16::MAX),
            "the catalog needs between 1 and {} heroes",
            u16::MAX
        );
        heroes
    })
}

impl Leader {
    pub fn all() -> impl Iterator<Item = Leader> {
        (0..heroes().len()).map(|index| Leader(index as u16))
    }

    pub fn count() -> usize {
        heroes().len()
    }

    /// Position among the catalog's heroes (stable for a given catalog).
    pub fn index(self) -> usize {
        usize::from(self.0)
    }

    pub fn nth(index: usize) -> Option<Leader> {
        (index < heroes().len()).then_some(Leader(index as u16))
    }

    /// The hero with this catalog id, for ids fixed in code (campaign
    /// advisors, tests). Panics when the catalog lost that hero.
    pub fn of(id: &str) -> Leader {
        Leader::all()
            .find(|leader| leader.id() == id)
            .unwrap_or_else(|| panic!("the catalog has no hero {id:?}"))
    }

    fn entry(self) -> &'static HeroEntry {
        &heroes()[self.index()]
    }

    pub fn character(self) -> &'static GeoEntity {
        self.entry().character
    }

    fn hero(self) -> &'static geo_entities::Hero {
        self.character()
            .hero
            .as_ref()
            .expect("catalog heroes have a hero entry")
    }

    /// Catalog id, saved in the database (`caesar`).
    pub fn id(self) -> &'static str {
        &self.character().id
    }

    /// Id in game messages and profiles (`Caesar`, `SunTzu`).
    pub fn wire_id(self) -> &'static str {
        &self.entry().wire_id
    }

    pub fn name(self) -> &'static str {
        &self.hero().name
    }

    /// The people or realm the hero leads.
    pub fn civilization(self) -> &'static str {
        &self.character().entity
    }

    pub fn filler_rgb(self) -> [f32; 3] {
        self.character()
            .territory_color_rgb()
            .expect("catalog heroes have a territory color")
    }

    /// Always playable without buying it.
    pub fn always_free(self) -> bool {
        self.hero().free
    }

    /// Canonical NFTROPOLY/WOU-ID card token; heroes not registered there have none.
    pub fn card_token_id(self) -> Option<&'static str> {
        self.hero().card.as_deref()
    }

    pub fn ability(self) -> Ability {
        self.hero().ability
    }

    /// `1 + percent/100` when this hero's ability matches, else `None`.
    fn ability_bonus(self, applies: impl Fn(Ability) -> bool) -> Option<f64> {
        let ability = self.ability();
        applies(ability).then(|| 1.0 + f64::from(ability.percent()) / 100.0)
    }

    /// `1 + percent/100` when this hero's ability matches, else `1`.
    fn ability_multiplier(self, applies: impl Fn(Ability) -> bool) -> f64 {
        self.ability_bonus(applies).unwrap_or(1.0)
    }

    pub fn troop_strength_multiplier(self) -> f64 {
        self.ability_multiplier(|a| matches!(a, Ability::CombatStrength { .. }))
    }

    pub fn expansion_speed_multiplier(self) -> f64 {
        self.ability_multiplier(|a| matches!(a, Ability::ExpansionSpeed { .. }))
    }

    /// `Some(bonus)` when this hero's district ability produces `resource` in `district`.
    /// Only that hero gets the hero-only base income for the pair.
    pub fn district_income_hero_bonus(
        self,
        district: geo_entities::District,
        resource: geo_entities::Resource,
    ) -> Option<f64> {
        self.ability_bonus(|a| {
            matches!(a, Ability::DistrictIncome { district: d, resource: r, .. }
                if d == district && r == resource)
        })
    }

    pub fn district_income_multiplier(
        self,
        district: geo_entities::District,
        resource: geo_entities::Resource,
    ) -> f64 {
        self.district_income_hero_bonus(district, resource)
            .unwrap_or(1.0)
    }

    pub fn city_troop_capacity_multiplier(self) -> f64 {
        self.ability_multiplier(|a| matches!(a, Ability::CityTroopCapacity { .. }))
    }

    pub fn bunker_range_multiplier(self) -> f64 {
        self.ability_multiplier(|a| matches!(a, Ability::BunkerRange { .. }))
    }

    pub fn bunker_defense_multiplier(self) -> f64 {
        self.ability_multiplier(|a| matches!(a, Ability::BunkerDefense { .. }))
    }

    pub fn elimination_gold_multiplier(self) -> f64 {
        self.ability_multiplier(|a| matches!(a, Ability::EliminationGold { .. }))
    }

    pub fn territory_gold_multiplier(self) -> f64 {
        self.ability_multiplier(|a| matches!(a, Ability::TerritoryGold { .. }))
    }

    pub fn enemy_troop_strength_multiplier(self) -> f64 {
        self.ability_multiplier(|a| matches!(a, Ability::EnemyTroopStrength { .. }))
    }

    pub fn damage_to_traitors_multiplier(self) -> f64 {
        self.ability_multiplier(|a| matches!(a, Ability::DamageToTraitors { .. }))
    }

    pub fn city_defense_multiplier(self) -> f64 {
        self.ability_multiplier(|a| matches!(a, Ability::CityDefense { .. }))
    }

    pub fn city_assault_multiplier(self) -> f64 {
        self.ability_multiplier(|a| matches!(a, Ability::CityAssault { .. }))
    }

    /// Share of a campaign alliance's gold price this hero pays (`1` unless discounted).
    pub fn alliance_cost_multiplier(self) -> f64 {
        match self.ability() {
            Ability::AllianceCost { percent } => 1.0 - f64::from(percent) / 100.0,
            _ => 1.0,
        }
    }

    /// `Some(bonus)` when every active alliance pays this hero gold.
    pub fn alliance_gold_hero_bonus(self) -> Option<f64> {
        self.ability_bonus(|a| matches!(a, Ability::AllianceGold { .. }))
    }
}

impl std::fmt::Debug for Leader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.wire_id())
    }
}

impl Serialize for Leader {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.wire_id())
    }
}

impl<'de> Deserialize<'de> for Leader {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        crate::commerce::leader_from_id(&value)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown hero {value:?}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_hero_ids_stay_readable() {
        for (wire, id, name) in [
            ("Caesar", "caesar", "Caesar"),
            ("SunTzu", "sun_tzu", "Sun Tzu"),
            (
                "RichardTheLionheart",
                "richard_the_lionheart",
                "Richard the Lionheart",
            ),
            ("LadySixSky", "lady_six_sky", "Lady Six Sky"),
        ] {
            let leader: Leader = serde_json::from_value(serde_json::json!(wire)).unwrap();
            assert_eq!(
                (leader.wire_id(), leader.id(), leader.name()),
                (wire, id, name)
            );
            assert_eq!(
                serde_json::to_value(leader).unwrap(),
                serde_json::json!(wire)
            );
        }
        assert_eq!(Leader::default().id(), "caesar");
    }

    #[test]
    fn every_hero_is_complete_in_the_catalog() {
        let mut names = std::collections::HashSet::new();
        for leader in Leader::all() {
            assert!(
                names.insert(leader.name()),
                "{leader:?} repeats a hero name"
            );
            assert!(
                !leader.civilization().is_empty(),
                "{leader:?} needs an entity"
            );
            assert!(
                leader.character().territory_color_rgb().is_some(),
                "{leader:?} needs a territory_color"
            );
        }
    }
}
