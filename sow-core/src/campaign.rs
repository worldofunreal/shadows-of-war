use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::game::BuildingKind;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CampaignAction {
    TransportFleet,
    TradeShip,
    Warship,
    Nuke,
}

impl CampaignAction {
    pub const ALL: [Self; 4] = [
        Self::TransportFleet,
        Self::TradeShip,
        Self::Warship,
        Self::Nuke,
    ];

    pub const fn required_building(self) -> Option<(BuildingKind, u8)> {
        match self {
            Self::TransportFleet => None,
            Self::TradeShip => Some((BuildingKind::Port, 1)),
            Self::Warship => Some((BuildingKind::Port, 2)),
            Self::Nuke => Some((BuildingKind::City, BuildingKind::City.max_level())),
        }
    }
}

/// Cumulative permissions for one offline campaign episode. Empty means locked.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct CampaignUnlocks {
    pub buildings: HashMap<BuildingKind, u8>,
    pub actions: Vec<CampaignAction>,
}

impl CampaignUnlocks {
    pub fn building_level(&self, kind: BuildingKind) -> u8 {
        self.buildings.get(&kind).copied().unwrap_or(0)
    }

    pub fn allows_building(&self, kind: BuildingKind, level: u8) -> bool {
        level > 0 && self.building_level(kind) >= level
    }

    pub fn allows_action(&self, action: CampaignAction) -> bool {
        self.actions.contains(&action)
            && action
                .required_building()
                .is_none_or(|(kind, level)| self.allows_building(kind, level))
    }

    pub fn is_valid(&self) -> bool {
        self.buildings
            .iter()
            .all(|(kind, level)| *level > 0 && *level <= kind.max_level())
            && {
                let mut seen = HashSet::with_capacity(self.actions.len());
                self.actions.iter().all(|action| seen.insert(*action))
            }
    }

    pub fn extends(&self, previous: &Self) -> bool {
        self.is_valid()
            && BuildingKind::ALL.into_iter().all(|kind| {
                self.buildings.get(&kind).copied().unwrap_or(0)
                    >= previous.buildings.get(&kind).copied().unwrap_or(0)
            })
            && previous
                .actions
                .iter()
                .all(|action| self.actions.contains(action))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn campaign_unlocks_are_closed_by_default_and_reject_invalid_or_revoked_access() {
        let locked = CampaignUnlocks::default();
        assert!(!locked.allows_building(BuildingKind::City, 1));
        assert!(!locked.allows_action(CampaignAction::TransportFleet));

        let city = CampaignUnlocks {
            buildings: HashMap::from([(BuildingKind::City, 1)]),
            actions: vec![],
        };
        assert!(city.is_valid());
        assert!(city.extends(&locked));

        let invalid_level = CampaignUnlocks {
            buildings: HashMap::from([(BuildingKind::City, 5)]),
            actions: vec![],
        };
        assert!(!invalid_level.is_valid());
        assert!(!locked.extends(&city));
    }

    #[test]
    fn action_unlocks_also_require_the_campaign_building_level() {
        let actions = vec![
            CampaignAction::TransportFleet,
            CampaignAction::TradeShip,
            CampaignAction::Warship,
            CampaignAction::Nuke,
        ];
        let granted = CampaignUnlocks {
            buildings: HashMap::from([(BuildingKind::Port, 1), (BuildingKind::City, 3)]),
            actions: actions.clone(),
        };
        assert!(granted.allows_action(CampaignAction::TradeShip));
        assert!(!granted.allows_action(CampaignAction::Warship));
        assert!(!granted.allows_action(CampaignAction::Nuke));
        assert!(CampaignAction::TransportFleet.required_building().is_none());

        let unlocked = CampaignUnlocks {
            buildings: HashMap::from([
                (BuildingKind::Port, 2),
                (BuildingKind::City, BuildingKind::City.max_level()),
            ]),
            actions,
        };
        assert!(
            CampaignAction::ALL
                .into_iter()
                .all(|action| unlocked.allows_action(action))
        );
    }
}
