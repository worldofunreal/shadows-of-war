use crate::bitset::DenseBitSet;
use crate::rng::NextIntExt;
use serde::{Deserialize, Serialize};

mod colors;

pub use colors::{
    best_available_nation_palette_color, campaign_relation_rgb, next_nation_palette_color,
    player_territory_rgb, team_territory_rgb,
};
pub use sow_data::{Leader, NamedColor, PREMIUM_COLORS};

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
    /// Unclaimed land (expansion targets); drawn from `assets/art/icons/wilderness`.
    Wilderness,
    Fallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvatarIdentityRef<'a> {
    Portrait {
        slug: &'a str,
        leader: Option<Leader>,
    },
    Fallback,
}

/// Humans show their hero; every bot carries its catalog portrait in
/// `campaign_avatar`, and a bot without one shows the shared fallback portrait.
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
        PlayerType::Bot | PlayerType::Nation => AvatarIdentityRef::Fallback,
    }
}

pub fn avatar_identity(player: &crate::protocol::PlayerSnapshot) -> AvatarIdentity {
    match avatar_identity_ref(player) {
        AvatarIdentityRef::Portrait { slug, leader } => AvatarIdentity::Portrait {
            slug: slug.to_owned(),
            leader,
        },
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
    pub active_emote: Option<crate::emote::ActiveEmote>,
    #[serde(default)]
    pub emote_event_id: u64,
    #[serde(default)]
    pub betrayal_retaliation_ticks: u32,
    /// Enemy that took one of this player's cities (0 = none). Only set while the leader has a recapture discount.
    #[serde(default)]
    pub recapture_target: u16,
    /// Ticks left in which attacks on `recapture_target` cost less.
    #[serde(default)]
    pub recapture_ticks: u32,
    #[serde(default)]
    pub traitor: bool,
    #[serde(default)]
    pub traitor_tick: u32,
    #[serde(default)]
    pub leader: Leader,
    /// People or realm a catalog bot is shown as (its catalog `entity`); `None` uses the hero's.
    #[serde(default)]
    pub realm: Option<String>,
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
    #[serde(default)]
    pub max_tile_count: u32,
    #[serde(default)]
    pub gold_received: f64,
    #[serde(default)]
    pub troops_received: f64,
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
            active_emote: None,
            emote_event_id: 0,
            betrayal_retaliation_ticks: 0,
            recapture_target: 0,
            recapture_ticks: 0,
            traitor: false,
            traitor_tick: 0,
            leader: Leader::default(),
            realm: None,
            skin_style: 0,
            kills: 0,
            deaths: 0,
            assists: 0,
            tile_conquests: std::collections::BTreeMap::new(),
            max_tile_count: 0,
            gold_received: config.starting_gold.max(0.0),
            troops_received: config.starting_troops.max(0.0),
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
        let leader = Leader::nth(rng.next_int(0, Leader::count() as i32) as usize)
            .expect("index drawn below the hero count");
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
            active_emote: None,
            emote_event_id: 0,
            betrayal_retaliation_ticks: 0,
            recapture_target: 0,
            recapture_ticks: 0,
            traitor: false,
            traitor_tick: 0,
            leader,
            realm: None,
            skin_style: 0,
            kills: 0,
            deaths: 0,
            assists: 0,
            tile_conquests: std::collections::BTreeMap::new(),
            max_tile_count: 0,
            gold_received: starting_gold.max(0.0),
            troops_received: starting_troops.max(0.0),
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
        let leader = Leader::nth(rng.next_int(0, Leader::count() as i32) as usize)
            .expect("index drawn below the hero count");
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
            active_emote: None,
            emote_event_id: 0,
            betrayal_retaliation_ticks: 0,
            recapture_target: 0,
            recapture_ticks: 0,
            traitor: false,
            traitor_tick: 0,
            leader,
            realm: None,
            skin_style: 0,
            kills: 0,
            deaths: 0,
            assists: 0,
            tile_conquests: std::collections::BTreeMap::new(),
            max_tile_count: 0,
            gold_received: config.starting_gold.max(0.0),
            troops_received: config.starting_troops.max(0.0),
        }
    }

    #[inline]
    pub fn credit_gold(&mut self, amount: f64) {
        self.gold += amount;
        if amount.is_finite() && amount > 0.0 {
            self.gold_received += amount;
        }
    }

    #[inline]
    pub fn credit_troops(&mut self, amount: f64) {
        self.troops += amount;
        if amount.is_finite() && amount > 0.0 {
            self.troops_received += amount;
        }
    }

    #[inline]
    pub fn credit_troops_capped(&mut self, amount: f64, max_troops: f64) {
        let previous = self.troops;
        self.troops = (self.troops + amount.max(0.0)).min(max_troops);
        let credited = (self.troops - previous).max(0.0);
        if credited.is_finite() {
            self.troops_received += credited;
        }
    }

    pub fn is_human(&self) -> bool {
        self.player_type == PlayerType::Human
    }
    /// The people or realm this player is shown as, the one hero `Kin`/`Foreign` targets
    /// compare: a catalog bot's own catalog entity, otherwise its hero's.
    pub fn realm(&self) -> &str {
        self.realm
            .as_deref()
            .unwrap_or_else(|| self.leader.civilization())
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

/// Name shown for a player; bots always carry their catalog name.
pub fn display_name(id: u16, name: &str, _player_type: PlayerType) -> String {
    if name.is_empty() {
        format!("Player {id}")
    } else {
        name.to_string()
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
            active_emote: None,
            can_express_emote: false,
            can_retaliate: false,
            traitor: false,
            leader: Leader::default(),
            empowered_attack: false,
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
        let tribe = player(2, "Mzilikazi", PlayerType::Bot, Some("ndebele"));
        let nation = player(3, "Hatshepsut", PlayerType::Nation, None);
        let campaign = player(4, "Tutorial", PlayerType::Bot, Some("boudica_campaign"));
        let missing_campaign = player(5, "Tutorial", PlayerType::Human, Some("null"));
        let players = [human, tribe, nation, campaign, missing_campaign];

        assert_eq!(
            avatar_identity_ref(&players[0]),
            AvatarIdentityRef::Portrait {
                slug: "caesar",
                leader: Some(Leader::of("caesar")),
            }
        );
        assert_eq!(
            avatar_identity_ref(&players[1]),
            AvatarIdentityRef::Portrait {
                slug: "ndebele",
                leader: None,
            }
        );
        assert_eq!(
            avatar_identity_ref(&players[2]),
            AvatarIdentityRef::Fallback
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
                leader: Some(Leader::of("caesar"))
            }
        );
        assert_eq!(
            avatar_identity_for_player_id(&players, 2),
            AvatarIdentity::Portrait {
                slug: "ndebele".into(),
                leader: None
            }
        );
        assert_eq!(
            avatar_identity_for_player_id(&players, 3),
            AvatarIdentity::Fallback
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
            Some(AvatarIdentity::Portrait {
                slug: "ndebele".into(),
                leader: None
            })
        );
        assert_eq!(repeated[1], None);
        assert_eq!(
            notification_avatar_identities(&players, [None, Some(3)])[1],
            Some(AvatarIdentity::Fallback)
        );
    }

    #[test]
    fn display_name_is_the_catalog_name_without_decorations() {
        assert_eq!(display_name(2, "Mzilikazi", PlayerType::Bot), "Mzilikazi");
        assert_eq!(
            display_name(3, "Hatshepsut", PlayerType::Nation),
            "Hatshepsut"
        );
        assert_eq!(display_name(7, "", PlayerType::Human), "Player 7");
    }

    #[test]
    fn avatar_payload_matches_the_hud_union_without_leaking_renderer_state() {
        assert_eq!(
            serde_json::to_value(AvatarIdentity::Portrait {
                slug: "caesar".into(),
                leader: Some(Leader::of("caesar")),
            })
            .unwrap(),
            serde_json::json!({ "kind": "portrait", "slug": "caesar" })
        );
        assert_eq!(
            serde_json::to_value(AvatarIdentity::Wilderness).unwrap(),
            serde_json::json!({ "kind": "wilderness" })
        );
        assert_eq!(
            serde_json::to_value(AvatarIdentity::Fallback).unwrap(),
            serde_json::json!({ "kind": "fallback" })
        );
    }
}

#[cfg(test)]
mod match_stats_tests {
    use super::Player;
    use crate::game_config::GameConfig;

    #[test]
    fn received_totals_include_starting_stock_and_survive_spending() {
        let config = GameConfig {
            starting_gold: 1_250.0,
            starting_troops: 2_000.0,
            ..GameConfig::default()
        };
        let mut player = Player::new_human(1, "Player".into(), [1.0; 3], &config);
        assert_eq!(player.gold_received, 1_250.0);
        assert_eq!(player.troops_received, 2_000.0);

        player.credit_gold(875.0);
        player.gold -= 1_500.0;
        player.credit_troops_capped(1_000.0, 2_500.0);
        player.troops -= 2_000.0;

        assert_eq!(player.gold_received, 2_125.0);
        assert_eq!(player.troops_received, 2_500.0);
    }
}
