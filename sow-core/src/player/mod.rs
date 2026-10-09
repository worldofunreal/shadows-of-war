use crate::bitset::DenseBitSet;
use crate::rng::NextIntExt;
use serde::{Deserialize, Serialize};
use sow_data::leader_for_civilization;

mod colors;

pub use colors::{
    bot_territory_color, campaign_relation_rgb, human_shader_territory_rgb, premium_color,
    team_territory_rgb,
};
pub use sow_data::{Civilization, Leader, NamedColor, PREMIUM_COLORS};

use wyrand::WyRand;

pub type PlayerId = u16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerType {
    Human,
    Bot,
    Nation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AvatarIdentity {
    Portrait {
        slug: String,
        #[serde(skip)]
        leader: Option<Leader>,
    },
    Emblem {
        symbol: &'static str,
    },
    Fallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvatarIdentityRef<'a> {
    Portrait {
        slug: &'a str,
        leader: Option<Leader>,
    },
    Emblem {
        symbol: &'static str,
    },
    Fallback,
}

pub fn avatar_identity_ref(player: &crate::protocol::PlayerSnapshot) -> AvatarIdentityRef<'_> {
    if let Some(slug) = player.campaign_avatar.as_deref() {
        if slug.is_empty() || slug == "null" {
            return AvatarIdentityRef::Fallback;
        }
        return AvatarIdentityRef::Portrait { slug, leader: None };
    }

    match player.player_type {
        PlayerType::Human => AvatarIdentityRef::Portrait {
            slug: sow_data::commerce::leader_id(player.leader),
            leader: Some(player.leader),
        },
        PlayerType::Bot => AvatarIdentityRef::Emblem {
            symbol: tribe_animal(player.id, &player.name),
        },
        PlayerType::Nation => AvatarIdentityRef::Emblem {
            symbol: empire_emoji(player.id, &player.name),
        },
    }
}

pub fn avatar_identity(player: &crate::protocol::PlayerSnapshot) -> AvatarIdentity {
    match avatar_identity_ref(player) {
        AvatarIdentityRef::Portrait { slug, leader } => AvatarIdentity::Portrait {
            slug: slug.to_owned(),
            leader,
        },
        AvatarIdentityRef::Emblem { symbol } => AvatarIdentity::Emblem { symbol },
        AvatarIdentityRef::Fallback => AvatarIdentity::Fallback,
    }
}

pub fn avatar_identity_for_player_id(
    players: &[crate::protocol::PlayerSnapshot],
    player_id: PlayerId,
) -> AvatarIdentity {
    players
        .iter()
        .find(|player| player.id == player_id)
        .map(avatar_identity)
        .unwrap_or(AvatarIdentity::Fallback)
}

pub fn notification_avatar_identities(
    players: &[crate::protocol::PlayerSnapshot],
    player_ids: [Option<PlayerId>; 2],
) -> [Option<AvatarIdentity>; 2] {
    let [first, second] = player_ids;
    [
        first.map(|player_id| avatar_identity_for_player_id(players, player_id)),
        second
            .filter(|player_id| first != Some(*player_id))
            .map(|player_id| avatar_identity_for_player_id(players, player_id)),
    ]
}

fn default_player_gold() -> f64 {
    crate::game_config::GameConfig::default().starting_gold
}

fn default_iq() -> u32 {
    100
}

fn default_iq_points() -> f64 {
    0.0
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Player {
    pub id: PlayerId,
    pub name: String,
    pub player_type: PlayerType,
    pub troops: f64,
    pub max_troops: f64,
    #[serde(default)]
    pub max_troops_cap: Option<f64>,
    #[serde(default = "default_player_gold")]
    pub gold: f64,
    pub color: [f32; 3],
    pub alive: bool,
    pub has_spawned: bool,
    pub sum_x: u64,
    pub sum_y: u64,
    pub tile_count: u32,
    pub border_tiles: DenseBitSet,
    #[serde(skip, default = "default_wyrand")]
    pub bot_rng: WyRand,
    pub factories: u32,
    pub cities: u32,
    pub team: Option<crate::protocol::Team>,
    #[serde(default = "default_iq")]
    pub iq: u32,
    #[serde(default = "default_iq_points")]
    pub iq_points: f64,
    #[serde(default)]
    pub alliances: Vec<PlayerId>,
    #[serde(default)]
    pub alliance_timers: std::collections::HashMap<PlayerId, u32>,
    #[serde(default)]
    pub disconnected: bool,
    /// Fictional humans: appear as Human players but the core auto-plays
    /// them via `execute_ai_think`, deterministically on every client. No
    /// sockets, no relay involvement — each client's engine simulates them.
    #[serde(default)]
    pub is_ai_controlled: bool,
    #[serde(default)]
    pub active_emoji: Option<String>,
    #[serde(default)]
    pub emoji_timer: u32,
    #[serde(default)]
    pub emoji_pinned: bool,
    #[serde(default)]
    pub traitor: bool,
    #[serde(default)]
    pub traitor_tick: u32,
    #[serde(default)]
    pub civilization: Civilization,
    #[serde(default)]
    pub leader: Leader,
    #[serde(default)]
    pub skin_style: u8,
    #[serde(default)]
    pub kills: u32,
    #[serde(default)]
    pub deaths: u32,
    #[serde(default)]
    pub assists: u32,
    #[serde(default)]
    pub tile_conquests: std::collections::BTreeMap<PlayerId, u32>,
}

fn default_wyrand() -> WyRand {
    WyRand::new(0)
}

impl Player {
    pub fn new_human(
        id: u16,
        name: String,
        color: [f32; 3],
        config: &crate::game_config::GameConfig,
    ) -> Self {
        Self {
            id,
            alive: true,
            player_type: PlayerType::Human,
            name,
            color,
            troops: config.starting_troops,
            max_troops: config.max_troops_base,
            max_troops_cap: None,
            gold: config.starting_gold,
            has_spawned: false,
            sum_x: 0,
            sum_y: 0,
            tile_count: 0,
            border_tiles: DenseBitSet::new(),
            bot_rng: WyRand::new(id as u64),
            factories: 0,
            cities: 0,
            team: None,
            iq: 100,
            iq_points: 0.0,
            alliances: Vec::new(),
            alliance_timers: std::collections::HashMap::new(),
            disconnected: false,
            is_ai_controlled: false,
            active_emoji: None,
            emoji_timer: 0,
            emoji_pinned: false,
            traitor: false,
            traitor_tick: 0,
            civilization: Civilization::Rome,
            leader: Leader::Caesar,
            skin_style: 0,
            kills: 0,
            deaths: 0,
            assists: 0,
            tile_conquests: std::collections::BTreeMap::new(),
        }
    }
    pub fn new_bot(
        id: u16,
        name: String,
        color: [f32; 3],
        config: &crate::game_config::GameConfig,
    ) -> Self {
        let mut rng = WyRand::new(id as u64);
        // Tribe (PlayerType::Bot) = lowest tier on the food chain (early-game
        // food). Single flat band — no id-based tiers, which used to mint
        // accidental "élite" tribes (id%100) that out-ranked nations.
        let iq = rng.next_int(50, 86) as u32;
        let civ = Civilization::ALL[rng.next_int(0, Civilization::ALL.len() as i32) as usize];
        let leader = leader_for_civilization(civ);
        let starting_troops = config.starting_troops * 0.5; // ponytail: tribes get half
        let starting_gold = if iq >= 130 {
            config.starting_gold
        } else if iq >= 100 {
            config.starting_gold * 0.5
        } else {
            0.0
        };
        Self {
            id,
            alive: true,
            player_type: PlayerType::Bot,
            name,
            color,
            troops: starting_troops,
            max_troops: config.max_troops_base,
            max_troops_cap: None,
            gold: starting_gold,
            has_spawned: false,
            sum_x: 0,
            sum_y: 0,
            tile_count: 0,
            border_tiles: DenseBitSet::new(),
            bot_rng: WyRand::new(id as u64),
            factories: 0,
            cities: 0,
            team: None,
            iq,
            iq_points: 0.0,
            alliances: Vec::new(),
            alliance_timers: std::collections::HashMap::new(),
            disconnected: false,
            is_ai_controlled: false,
            active_emoji: None,
            emoji_timer: 0,
            emoji_pinned: false,
            traitor: false,
            traitor_tick: 0,
            civilization: civ,
            leader,
            skin_style: 0,
            kills: 0,
            deaths: 0,
            assists: 0,
            tile_conquests: std::collections::BTreeMap::new(),
        }
    }
    pub fn new_nation(
        id: u16,
        name: String,
        color: [f32; 3],
        config: &crate::game_config::GameConfig,
    ) -> Self {
        let mut rng = WyRand::new(id as u64);
        // Nation (PlayerType::Nation) = second tier on the food chain
        // (mid-game food). Strictly below ghosts and above tribes. Band 130-159.
        let iq = rng.next_int(130, 160) as u32;
        let civ = Civilization::ALL[rng.next_int(0, Civilization::ALL.len() as i32) as usize];
        let leader = leader_for_civilization(civ);
        let final_color = color;
        Self {
            id,
            alive: true,
            player_type: PlayerType::Nation,
            name,
            color: final_color,
            troops: config.starting_troops,
            max_troops: config.max_troops_base,
            max_troops_cap: None,
            gold: config.starting_gold,
            has_spawned: false,
            sum_x: 0,
            sum_y: 0,
            tile_count: 0,
            border_tiles: DenseBitSet::new(),
            bot_rng: WyRand::new(id as u64),
            factories: 0,
            cities: 0,
            team: None,
            iq,
            iq_points: 0.0,
            alliances: Vec::new(),
            alliance_timers: std::collections::HashMap::new(),
            disconnected: false,
            is_ai_controlled: false,
            active_emoji: None,
            emoji_timer: 0,
            emoji_pinned: false,
            traitor: false,
            traitor_tick: 0,
            civilization: civ,
            leader,
            skin_style: 0,
            kills: 0,
            deaths: 0,
            assists: 0,
            tile_conquests: std::collections::BTreeMap::new(),
        }
    }
    pub fn is_human(&self) -> bool {
        self.player_type == PlayerType::Human
    }
    pub fn border_coords(&self, map_width: u32) -> impl Iterator<Item = (u32, u32)> + '_ {
        self.border_tiles
            .ones()
            .map(move |idx| (idx % map_width, idx / map_width))
    }
    #[inline]
    pub fn border_insert(&mut self, idx: u32) {
        self.border_tiles.insert(idx);
    }
    #[inline]
    pub fn border_remove(&mut self, idx: u32) {
        self.border_tiles.remove(idx);
    }
}

pub fn tribe_animal(id: u16, name: &str) -> &'static str {
    if name.is_empty() {
        sow_data::animal_for_id(id)
    } else {
        sow_data::animal_for_name(name)
    }
}

/// Empire/nation emoji — same selection scheme as [`tribe_animal`] but a distinct category.
pub fn empire_emoji(id: u16, name: &str) -> &'static str {
    if name.is_empty() {
        sow_data::empire_emoji_for_id(id)
    } else {
        sow_data::empire_emoji_for_name(name)
    }
}

pub fn display_name(id: u16, name: &str, player_type: PlayerType) -> String {
    if name.is_empty() {
        match player_type {
            PlayerType::Bot => format!(
                "{} Tribe {}",
                tribe_animal(id, name),
                id.saturating_sub(199)
            ),
            PlayerType::Nation => format!("Nation {}", id.saturating_sub(103)),
            PlayerType::Human => format!("Player {}", id),
        }
    } else {
        match player_type {
            PlayerType::Bot => format!("{} {}", tribe_animal(id, name), name),
            _ => name.to_string(),
        }
    }
}

#[cfg(test)]
mod avatar_identity_tests {
    use super::*;
    use crate::protocol::PlayerSnapshot;

    fn player(
        id: PlayerId,
        name: &str,
        player_type: PlayerType,
        campaign_avatar: Option<&str>,
    ) -> PlayerSnapshot {
        PlayerSnapshot {
            id,
            name: name.to_owned(),
            troops: 0.0,
            max_troops: 0.0,
            gold: 0.0,
            tile_count: 0,
            centroid_x: 0.0,
            centroid_y: 0.0,
            player_type,
            color: [0.0; 3],
            team: None,
            has_spawned: true,
            alive: true,
            iq: 100,
            alliances: Vec::new(),
            alliance_timers: Default::default(),
            alliance_requests: Vec::new(),
            resource_requests: Vec::new(),
            disconnected: false,
            active_emoji: None,
            traitor: false,
            civilization: Civilization::Rome,
            leader: Leader::Caesar,
            campaign_avatar: campaign_avatar.map(str::to_owned),
            is_campaign_faction: false,
            skin_style: 0,
            kills: 0,
            deaths: 0,
            assists: 0,
            boats_in_use: 0,
            boat_capacity: 0,
            nuke_available: false,
            nuke_cooldown_ticks: 0,
        }
    }

    #[test]
    fn resolves_each_entity_from_its_own_identity_and_falls_back_only_when_missing() {
        let human = player(1, "Player", PlayerType::Human, None);
        let tribe = player(2, "Trinovantes", PlayerType::Bot, None);
        let nation = player(3, "Rome", PlayerType::Nation, None);
        let campaign = player(4, "Tutorial", PlayerType::Bot, Some("boudica_campaign"));
        let missing_campaign = player(5, "Tutorial", PlayerType::Human, Some("null"));
        let players = [human, tribe, nation, campaign, missing_campaign];

        assert_eq!(
            avatar_identity_ref(&players[0]),
            AvatarIdentityRef::Portrait {
                slug: "caesar",
                leader: Some(Leader::Caesar),
            }
        );
        assert_eq!(
            avatar_identity_ref(&players[1]),
            AvatarIdentityRef::Emblem {
                symbol: tribe_animal(2, "Trinovantes"),
            }
        );
        assert_eq!(
            avatar_identity_ref(&players[2]),
            AvatarIdentityRef::Emblem {
                symbol: empire_emoji(3, "Rome"),
            }
        );
        assert_eq!(
            avatar_identity_ref(&players[3]),
            AvatarIdentityRef::Portrait {
                slug: "boudica_campaign",
                leader: None,
            }
        );
        assert_eq!(
            avatar_identity_ref(&players[4]),
            AvatarIdentityRef::Fallback
        );

        assert_eq!(
            avatar_identity_for_player_id(&players, 1),
            AvatarIdentity::Portrait {
                slug: "caesar".into(),
                leader: Some(Leader::Caesar)
            }
        );
        assert_eq!(
            avatar_identity_for_player_id(&players, 2),
            AvatarIdentity::Emblem {
                symbol: tribe_animal(2, "Trinovantes")
            }
        );
        assert_eq!(
            avatar_identity_for_player_id(&players, 3),
            AvatarIdentity::Emblem {
                symbol: empire_emoji(3, "Rome")
            }
        );
        assert_eq!(
            avatar_identity_for_player_id(&players, 4),
            AvatarIdentity::Portrait {
                slug: "boudica_campaign".into(),
                leader: None
            }
        );
        assert_eq!(
            avatar_identity_for_player_id(&players, 5),
            AvatarIdentity::Fallback
        );
        assert_eq!(
            avatar_identity_for_player_id(&players, 99),
            AvatarIdentity::Fallback
        );
        let repeated = notification_avatar_identities(&players, [Some(2), Some(2)]);
        assert_eq!(
            repeated[0],
            Some(AvatarIdentity::Emblem {
                symbol: tribe_animal(2, "Trinovantes")
            })
        );
        assert_eq!(repeated[1], None);
        assert_eq!(
            notification_avatar_identities(&players, [None, Some(3)])[1],
            Some(AvatarIdentity::Emblem {
                symbol: empire_emoji(3, "Rome")
            })
        );
    }

    #[test]
    fn avatar_payload_matches_the_hud_union_without_leaking_renderer_state() {
        assert_eq!(
            serde_json::to_value(AvatarIdentity::Portrait {
                slug: "caesar".into(),
                leader: Some(Leader::Caesar),
            })
            .unwrap(),
            serde_json::json!({ "kind": "portrait", "slug": "caesar" })
        );
        assert_eq!(
            serde_json::to_value(AvatarIdentity::Emblem { symbol: "🐺" }).unwrap(),
            serde_json::json!({ "kind": "emblem", "symbol": "🐺" })
        );
        assert_eq!(
            serde_json::to_value(AvatarIdentity::Fallback).unwrap(),
            serde_json::json!({ "kind": "fallback" })
        );
    }
}
