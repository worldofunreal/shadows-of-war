//! Playable heroes. A hero is a catalog character (`assets/geo_entities.json`)
//! with a `hero` entry: adding one there adds it to the game, the store and
//! the menu, and `hero_ability_tests` proves its ability works.
use crate::geo_entities::{self, Ability, GeoEntity, LossWhen, Side, Source, Target};
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

    /// Asset filename slug, independent of the saved hero id.
    pub fn art_slug(self) -> &'static str {
        self.hero().art_slug.as_deref().unwrap_or_else(|| self.id())
    }

    /// Id in game messages and profiles (`Caesar`, `SunTzu`).
    pub fn wire_id(self) -> &'static str {
        &self.entry().wire_id
    }

    pub fn name(self) -> &'static str {
        &self.hero().name
    }

    pub fn display_name(self) -> &'static str {
        &self.character().name
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

    /// `1 + percent/100` on attack strength against `target`, else `1`.
    pub fn attack_strength_multiplier(self, target: Target) -> f64 {
        self.ability_multiplier(
            |a| matches!(a, Ability::AttackStrength { against, .. } if against == target),
        )
    }

    /// `1 + percent/100` on conquest speed against `target`, else `1`.
    pub fn conquest_speed_multiplier(self, target: Target) -> f64 {
        self.ability_multiplier(
            |a| matches!(a, Ability::ConquestSpeed { against, .. } if against == target),
        )
    }

    /// Whether this hero's attacks gain strength, speed or fewer losses against `target`.
    pub fn attack_ability_targets(self, target: Target) -> bool {
        matches!(
            self.ability(),
            Ability::AttackStrength { against, .. }
                | Ability::ConquestSpeed { against, .. }
                | Ability::AttackerLoss { when: LossWhen::Against(against), .. }
                if against == target
        )
    }

    /// `Some(bonus)` when this hero's `source` pays `resource` (its hero-only base).
    pub fn resource_bonus_hero_bonus(
        self,
        source: Source,
        resource: geo_entities::Resource,
    ) -> Option<f64> {
        self.ability_bonus(|a| {
            matches!(a, Ability::ResourceBonus { source: s, resource: r, .. }
                if s == source && r == resource)
        })
    }

    /// `1 + percent/100` when this hero's `source` pays `resource`, else `1`.
    pub fn resource_bonus_multiplier(
        self,
        source: Source,
        resource: geo_entities::Resource,
    ) -> f64 {
        self.resource_bonus_hero_bonus(source, resource).unwrap_or(1.0)
    }

    pub fn city_troop_capacity_multiplier(self) -> f64 {
        self.ability_multiplier(|a| matches!(a, Ability::CityTroopCapacity { .. }))
    }

    pub fn tower_range_multiplier(self) -> f64 {
        let ability_percent = match self.ability() {
            Ability::TowerRange { percent } => percent,
            _ => 0,
        };
        let percent = ability_percent.max(self.hero().tower_range_bonus_percent);
        1.0 + f64::from(percent) / 100.0
    }

    /// Tower fights for `side`: a defender gets `1 + percent/100` (attackers lose
    /// more), an attacker gets `1 - percent/100` (its armies lose less), else `1`.
    pub fn tower_loss_multiplier(self, side: Side) -> f64 {
        match self.ability() {
            Ability::TowerLoss { side: s, percent } if s == side => match side {
                Side::Defender => 1.0 + f64::from(percent) / 100.0,
                Side::Attacker => 1.0 - f64::from(percent) / 100.0,
            },
            _ => 1.0,
        }
    }

    /// `1 + percent/100` on city-tile capture for `side`, else `1`.
    pub fn city_capture_multiplier(self, side: Side) -> f64 {
        self.ability_multiplier(
            |a| matches!(a, Ability::CityCapture { side: s, .. } if s == side),
        )
    }

    /// `1 - percent/100` on the troops this hero's armies lose when `when` holds, else `1`.
    pub fn attacker_loss_multiplier(self, when: LossWhen) -> f64 {
        match self.ability() {
            Ability::AttackerLoss { when: w, percent } if w == when => {
                1.0 - f64::from(percent) / 100.0
            }
            _ => 1.0,
        }
    }

    /// Whether this hero has the attacker-loss ability for `when`.
    pub fn has_attacker_loss(self, when: LossWhen) -> bool {
        matches!(self.ability(), Ability::AttackerLoss { when: w, .. } if w == when)
    }

    /// Whether this hero's ability pays from `source` (gold or troops).
    pub fn has_resource_bonus_from(self, source: Source) -> bool {
        matches!(self.ability(), Ability::ResourceBonus { source: s, .. } if s == source)
    }

    /// `1 + percent/100` on the troops a `target` army loses when retreating from this hero's land, else `1`.
    pub fn enemy_retreat_penalty_multiplier(self, target: Target) -> f64 {
        self.ability_multiplier(
            |a| matches!(a, Ability::EnemyRetreatPenalty { against, .. } if against == target),
        )
    }

    /// Share of a campaign alliance's gold price with `target` this hero pays (`1` unless discounted).
    pub fn alliance_cost_multiplier(self, target: Target) -> f64 {
        match self.ability() {
            Ability::AllianceCost { against, percent } if against == target => {
                1.0 - f64::from(percent) / 100.0
            }
            _ => 1.0,
        }
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

    #[test]
    fn tower_range_bonuses_keep_other_hero_effects() {
        assert_eq!(Leader::of("kamehameha").tower_range_multiplier(), 1.5);
        assert_eq!(
            Leader::of("kamehameha").enemy_retreat_penalty_multiplier(Target::OtherPlayer),
            2.0
        );
        assert_eq!(
            Leader::of("leonidas").tower_loss_multiplier(Side::Defender),
            1.5
        );
        assert_eq!(
            Leader::of("catherine_the_great").tower_loss_multiplier(Side::Attacker),
            0.5
        );
    }
}
