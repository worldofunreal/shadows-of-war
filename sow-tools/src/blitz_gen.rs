use serde_json::json;
use sow_core::engine::initialize_match_engine_with_water;
use sow_core::game::{BuildingKind, GamePhase};
use sow_core::game_config::{CampaignSupport, GameConfig, ScriptedSpawn};
use sow_core::map_file::{self, MapFile};
use sow_core::player::{Leader, PlayerType};
use sow_core::protocol::{
    AttackIntent, CampaignRelation, GameplayIntent, PlayerInfo, StampedIntent, Team,
};
use std::collections::VecDeque;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

pub struct GenerateBlitzArgs {
    pub campaign_root: PathBuf,
    pub maps_root: PathBuf,
    pub start: u32,
    pub count: u32,
    pub dry_run: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SawtoothArchetype {
    Boss,
    Cascade,
    Flank,
    EcoRush,
    Tense,
}

impl SawtoothArchetype {
    pub fn from_node(node: u32) -> Self {
        match node % 5 {
            0 => Self::Boss,
            1 => Self::Cascade,
            2 => Self::Flank,
            3 => Self::EcoRush,
            _ => Self::Tense,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Boss => "Boss",
            Self::Cascade => "Cascade",
            Self::Flank => "Flank",
            Self::EcoRush => "Eco Rush",
            Self::Tense => "High Stakes",
        }
    }
}

const MAP_ROTATION: &[&str] = &[
    "europe",
    "bajacalifornia",
    "southeastasia",
    "africa",
    "northamerica",
];

/// Inspect contiguous land tiles directly from parsed `MapFile` terrain via BFS around candidate anchors.
fn extract_terrain(raw: &[u8], map: &MapFile) -> Option<Vec<u8>> {
    // `map_file::parse` returns terrain length = w * h; locate terrain slice safely via parse_header logic:
    let mut off = 4 + 2 + 2 + 12;
    let dlen = u16::from_le_bytes([*raw.get(off)?, *raw.get(off + 1)?]) as usize;
    off += 2 + dlen;
    let scount = u16::from_le_bytes([*raw.get(off)?, *raw.get(off + 1)?]) as usize;
    off += 2;
    for _ in 0..scount {
        let nl = u16::from_le_bytes([*raw.get(off)?, *raw.get(off + 1)?]) as usize;
        off += 2 + nl;
        let fl = u16::from_le_bytes([*raw.get(off)?, *raw.get(off + 1)?]) as usize;
        off += 2 + fl + 8;
    }
    let len = map.width as usize * map.height as usize;
    raw.get(off..off + len).map(|s| s.to_vec())
}

fn pick_connected_cluster(map: &MapFile, terrain: &[u8], seed_offset: usize) -> Option<Vec<(u32, u32)>> {
    let w = map.width;
    let h = map.height;
    let land = |x: u32, y: u32| -> bool {
        x < w && y < h && (terrain[y as usize * w as usize + x as usize] & 0x80) != 0
    };

    let mut seeds: Vec<(u32, u32)> = map
        .spawns
        .iter()
        .filter(|s| land(s.x, s.y))
        .map(|s| (s.x, s.y))
        .collect();
    if seeds.is_empty() {
        for y in (48..h.saturating_sub(48)).step_by(32) {
            for x in (48..w.saturating_sub(48)).step_by(32) {
                if land(x, y) {
                    seeds.push((x, y));
                }
            }
        }
    }
    if seeds.is_empty() {
        return None;
    }

    for shift in 0..seeds.len() {
        let root = seeds[(seed_offset + shift) % seeds.len()];
        let mut dist = vec![u32::MAX; (w * h) as usize];
        let mut q = VecDeque::new();
        let ridx = (root.1 * w + root.0) as usize;
        dist[ridx] = 0;
        q.push_back(root);
        let mut ring1 = Vec::new();
        let mut ring2 = Vec::new();
        let mut ring3 = Vec::new();
        let mut ally_ring = Vec::new();

        while let Some((cx, cy)) = q.pop_front() {
            let cd = dist[(cy * w + cx) as usize];
            if cd >= 58 {
                continue;
            }
            if (14..=24).contains(&cd) && cx % 4 == 0 && cy % 4 == 0 {
                if ring1.is_empty() {
                    ring1.push((cx, cy));
                } else if ally_ring.is_empty() && cx.abs_diff(ring1[0].0) + cy.abs_diff(ring1[0].1) >= 16 {
                    ally_ring.push((cx, cy));
                }
            } else if (26..=38).contains(&cd) && cx % 4 == 0 && cy % 4 == 0 && ring2.is_empty() {
                if ring1.first().is_none_or(|r| cx.abs_diff(r.0) + cy.abs_diff(r.1) >= 12) {
                    ring2.push((cx, cy));
                }
            } else if (40..=55).contains(&cd) && cx % 4 == 0 && cy % 4 == 0 && ring3.is_empty() {
                ring3.push((cx, cy));
            }
            for (nx, ny) in [
                (cx.wrapping_sub(1), cy),
                (cx + 1, cy),
                (cx, cy.wrapping_sub(1)),
                (cx, cy + 1),
            ] {
                if nx < w && ny < h && land(nx, ny) {
                    let nidx = (ny * w + nx) as usize;
                    if dist[nidx] == u32::MAX {
                        dist[nidx] = cd + 1;
                        q.push_back((nx, ny));
                    }
                }
            }
        }
        if let (Some(&t1), Some(&t2), Some(&boss)) = (ring1.first(), ring2.first(), ring3.first()) {
            let ally = ally_ring.first().copied().unwrap_or((
                root.0.saturating_sub(12).max(4),
                root.1.saturating_sub(8).max(4),
            ));
            return Some(vec![root, t1, t2, boss, ally]);
        }
    }
    None
}

fn simulate_headless(
    map_bytes: &[u8],
    map: &MapFile,
    player_spawn: (u32, u32),
    starting_troops: f64,
    spawns: Vec<ScriptedSpawn>,
    campaign_support: Option<CampaignSupport>,
    blind_frontal: bool,
) -> (bool, u64) {
    let mut cfg = GameConfig::default();
    cfg.map_width = map.width;
    cfg.map_height = map.height;
    cfg.tutorial = true;
    cfg.random_spawn = true;
    cfg.buildings_enabled = true;
    cfg.starting_troops = starting_troops;
    cfg.starting_gold = 300.0;
    cfg.player_leader = Leader::of("caesar");
    cfg.player_spawn = Some(player_spawn);
    cfg.scripted_spawns = spawns;
    cfg.campaign_support = campaign_support;

    let players = vec![PlayerInfo {
        id: 1,
        name: "Player".to_string(),
        player_type: PlayerType::Human,
        color: [0.94, 0.55, 0.16],
        team: None,
        spawn_x: player_spawn.0,
        spawn_y: player_spawn.1,
        leader: cfg.player_leader,
        skin_style: 0,
        is_ai_controlled: false,
    }];
    let mut engine = initialize_match_engine_with_water(
        cfg,
        0xB117_2027,
        map_bytes,
        players,
        map.spawns.clone(),
        map.geo_bounds,
        map.rosters.clone(),
        map.num_land_tiles,
        None,
    );

    let my_id = 1u16;
    let mut attack_step = 0u8;
    for t in 1..=800u64 {
        engine.tick();
        if t % 15 == 5 && engine.state.phase == GamePhase::Playing {
            let mut hostiles: Vec<(u16, f64)> = engine
                .state
                .players
                .iter()
                .filter(|p| {
                    p.alive
                        && p.id != my_id
                        && engine.campaign_relations.get(&p.id) != Some(&CampaignRelation::Allied)
                })
                .map(|p| (p.id, p.troops))
                .collect();
            hostiles.sort_by(|a, b| a.1.total_cmp(&b.1));
            if let Some(&(weak_id, _)) = hostiles.first()
                && let Some(&(strong_id, _)) = hostiles.last()
            {
                let target = if blind_frontal {
                    strong_id
                } else {
                    if attack_step == 0 {
                        attack_step = 1;
                        0
                    } else {
                        weak_id
                    }
                };
                let frac = engine
                    .state
                    .player(my_id)
                    .map(|p| p.troops * 0.55)
                    .unwrap_or(0.0);
                if frac >= 50.0 {
                    engine.apply_intents(&[StampedIntent {
                        player_id: my_id,
                        intent: GameplayIntent::Attack(AttackIntent {
                            target_owner: target,
                            troops: Some(frac),
                            target_tile: None,
                        }),
                    }]);
                }
            }
        }
        let boss_alive = engine.state.players.iter().any(|p| {
            p.alive
                && p.id != my_id
                && engine.campaign_relations.get(&p.id) != Some(&CampaignRelation::Allied)
        });
        let player_alive = engine.state.player(my_id).is_some_and(|p| p.alive);
        if !player_alive {
            return (false, t);
        }
        if !boss_alive {
            return (true, t);
        }
    }
    let boss_alive = engine.state.players.iter().any(|p| {
        p.alive
            && p.id != my_id
            && engine.campaign_relations.get(&p.id) != Some(&CampaignRelation::Allied)
    });
    (!boss_alive, 800)
}

#[allow(clippy::too_many_arguments)]
fn make_scripted_spawn(
    faction_id: &str,
    name: String,
    pos: (u32, u32),
    color: [f32; 3],
    leader: Leader,
    troops: u32,
    iq: u32,
    player_type: Option<PlayerType>,
    can_attack_player: bool,
    gold_loot_bonus: Option<u32>,
    gold_loot_override: Option<u32>,
    troop_loot_bonus: Option<u32>,
    prebuilt_buildings: Vec<(BuildingKind, u8)>,
) -> ScriptedSpawn {
    ScriptedSpawn {
        name,
        x: pos.0,
        y: pos.1,
        color,
        team: Some(Team::RED),
        leader,
        troops: Some(f64::from(troops)),
        iq: Some(iq),
        campaign_avatar: None,
        campaign_support_interval_seconds: None,
        campaign_gold_loot_bonus: gold_loot_bonus,
        campaign_gold_loot_override: gold_loot_override,
        campaign_alliance_group: None,
        campaign_relation: Some(CampaignRelation::Enemy),
        campaign_faction_id: Some(faction_id.to_string()),
        campaign_can_request_alliance: Some(false),
        campaign_can_attack_player: Some(can_attack_player),
        campaign_assault_force: false,
        campaign_hold_structures_until_contact: false,
        campaign_accepts_alliance_requests: false,
        player_type,
        campaign_troop_loot_bonus: troop_loot_bonus,
        prebuilt_buildings,
    }
}

pub fn run(args: GenerateBlitzArgs) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(&args.campaign_root)?;
    for offset in 0..args.count {
        let node = args.start + offset;
        let episode_id = format!("blitz_{node:02}");
        let map_name = MAP_ROTATION[(node.saturating_sub(5) as usize) % MAP_ROTATION.len()];
        let map_path = args.maps_root.join(map_name).join("map.bin");
        let raw = fs::read(&map_path)?;
        let map = map_file::parse(&raw)?;
        let terrain = extract_terrain(&raw, &map)
            .ok_or_else(|| format!("Failed to extract terrain for {map_name}"))?;
        let cluster = pick_connected_cluster(&map, &terrain, offset as usize)
            .ok_or_else(|| format!("Failed to find connected land cluster on {map_name}"))?;

        let arch = SawtoothArchetype::from_node(node);
        let diff = (node.saturating_sub(4)) as f64;
        let player_spawn = cluster[0];
        let mut player_troops = (1400.0 + diff * 120.0).round() as u32;
        let mut pinata1_loot = (1600.0 + diff * 180.0).round() as u32;
        let pinata1_troops = (550.0 + diff * 45.0).round() as u32;
        let pinata2_troops = (900.0 + diff * 80.0).round() as u32;
        let mut boss_troops = match arch {
            SawtoothArchetype::Boss => (3800.0 + diff * 260.0).round() as u32,
            SawtoothArchetype::Tense => (3200.0 + diff * 220.0).round() as u32,
            SawtoothArchetype::Cascade => (2400.0 + diff * 180.0).round() as u32,
            _ => (2800.0 + diff * 200.0).round() as u32,
        };

        let pinata_buildings = match arch {
            SawtoothArchetype::EcoRush => vec![(BuildingKind::TradingPost, 1)],
            SawtoothArchetype::Cascade => vec![(BuildingKind::TradingPost, 1)],
            _ => vec![(BuildingKind::City, 1)],
        };
        let boss_buildings = match arch {
            SawtoothArchetype::Boss | SawtoothArchetype::Tense => {
                vec![(BuildingKind::City, 1), (BuildingKind::Tower, 1)]
            }
            _ => vec![(BuildingKind::City, 1)],
        };

        // Iteratively tune with headless SowEngine dual simulations.
        for _ in 0..5 {
            let spawns = vec![
                make_scripted_spawn(
                    "outpost_alpha",
                    "Outpost Alpha".into(),
                    cluster[1],
                    [0.85, 0.33, 0.31],
                    Leader::of("ragnar"),
                    pinata1_troops,
                    65,
                    None,
                    false,
                    Some(120),
                    Some(180),
                    Some(pinata1_loot),
                    pinata_buildings.clone(),
                ),
                make_scripted_spawn(
                    "vanguard_keep",
                    "Vanguard Keep".into(),
                    cluster[2],
                    [0.83, 0.51, 0.16],
                    Leader::of("alexander"),
                    pinata2_troops,
                    75,
                    None,
                    arch != SawtoothArchetype::Cascade,
                    Some(150),
                    Some(220),
                    Some((pinata1_loot as f64 * 1.15) as u32),
                    vec![(BuildingKind::TradingPost, 1)],
                ),
                make_scripted_spawn(
                    "sector_boss",
                    format!("Warlord Sector {node}"),
                    cluster[3],
                    [0.66, 0.20, 0.20],
                    Leader::of("richard_the_lionheart"),
                    boss_troops,
                    95,
                    Some(PlayerType::Nation),
                    true,
                    None,
                    None,
                    None,
                    boss_buildings.clone(),
                ),
            ];
            let (blind_won, blind_ticks) = simulate_headless(
                &raw,
                &map,
                player_spawn,
                f64::from(player_troops),
                spawns.clone(),
                None,
                true,
            );
            let (opt_won, opt_ticks) = simulate_headless(
                &raw,
                &map,
                player_spawn,
                f64::from(player_troops),
                spawns,
                None,
                false,
            );
            if (!blind_won || blind_ticks > 280) && opt_won && (90..=680).contains(&opt_ticks) {
                break;
            }
            if blind_won && blind_ticks <= 280 {
                boss_troops += 500;
            }
            if !opt_won || opt_ticks > 680 {
                player_troops += 220;
                pinata1_loot += 400;
            }
        }

        let mut factions = vec![
            json!({
                "id": "outpost_alpha",
                "name": "Outpost Alpha",
                "x": cluster[1].0,
                "y": cluster[1].1,
                "starting_troops": pinata1_troops,
                "relation": "enemy",
                "team": "Red",
                "can_request_alliance": false,
                "color": "#d9534f",
                "iq": 65,
                "civ": "Raiders",
                "leader": "Ragnar",
                "avatar": "ragnar",
                "can_attack_player": false,
                "gold_loot_override": 180,
                "troop_loot_bonus": pinata1_loot,
                "prebuilt_buildings": pinata_buildings.iter().map(|(k, l)| json!([format!("{k:?}"), l])).collect::<Vec<_>>()
            }),
            json!({
                "id": "vanguard_keep",
                "name": "Vanguard Keep",
                "x": cluster[2].0,
                "y": cluster[2].1,
                "starting_troops": pinata2_troops,
                "relation": "enemy",
                "team": "Red",
                "can_request_alliance": false,
                "color": "#d4822a",
                "iq": 75,
                "civ": "Warband",
                "leader": "Alexander",
                "avatar": "alexander",
                "can_attack_player": arch != SawtoothArchetype::Cascade,
                "gold_loot_override": 220,
                "troop_loot_bonus": (pinata1_loot as f64 * 1.15) as u32,
                "prebuilt_buildings": [["TradingPost", 1]]
            }),
            json!({
                "id": "sector_boss",
                "name": format!("Sector {node} Warlord"),
                "x": cluster[3].0,
                "y": cluster[3].1,
                "starting_troops": boss_troops,
                "relation": "enemy",
                "team": "Red",
                "can_request_alliance": false,
                "color": "#a83232",
                "iq": 95,
                "civ": "Citadel",
                "leader": "Richard",
                "avatar": "richard",
                "can_attack_player": true,
                "prebuilt_buildings": boss_buildings.iter().map(|(k, l)| json!([format!("{k:?}"), l])).collect::<Vec<_>>()
            }),
        ];

        let mut settings = json!({
            "starting_troops": player_troops,
            "starting_gold": 300 + node * 25
        });

        if arch == SawtoothArchetype::Flank {
            factions.insert(
                0,
                json!({
                    "id": "allied_vanguard",
                    "name": "Allied Vanguard",
                    "x": cluster[4].0,
                    "y": cluster[4].1,
                    "starting_troops": 3500,
                    "relation": "allied",
                    "can_request_alliance": true,
                    "accepts_alliance_requests": true,
                    "support_interval_seconds": 8,
                    "color": "#3aa675",
                    "iq": 100,
                    "civ": "Merchant League",
                    "leader": "Musa",
                    "avatar": "musa",
                    "can_attack_player": false
                }),
            );
            settings["campaign_support"] = json!({
                "after_defeated": "immediate",
                "share_percent": 45
            });
        }

        let roster_json = json!({
            "_comment": format!("Conquest Blitz Node {node} ({}) on {map_name}", arch.label()),
            "map": map_name,
            "player_spawn": [player_spawn.0, player_spawn.1],
            "player_color": "#f08c28",
            "factions": factions
        });

        let triggers_json = json!({
            "version": 2,
            "episode_id": episode_id,
            "text_namespace": "tutorial.blitz_",
            "require_speaker_portraits": false,
            "settings": settings,
            "entry": "blitz_cascade_start",
            "speakers": {
                "caesar": {
                    "name_key": "tutorial.blitz_advisor_name",
                    "avatar": "caesar"
                },
                "sector_boss": {
                    "name_key": "tutorial.blitz_advisor_name",
                    "faction": "sector_boss"
                }
            },
            "steps": [
                {
                    "id": "blitz_cascade_start",
                    "type": "objective",
                    "unlocks": {
                        "buildings": { "City": 1, "TradingPost": 2, "Tower": 2 }
                    },
                    "title_key": "tutorial.blitz_obj_title",
                    "body_key": "tutorial.blitz_obj_body",
                    "hint_key": "tutorial.blitz_obj_hint",
                    "speaker": "caesar",
                    "trigger": {
                        "type": "defeated",
                        "target": "outpost_alpha",
                        "scope": "episode"
                    },
                    "marker": { "target": "outpost_alpha" },
                    "guide": { "kind": "world", "target": "target_action", "gesture": "tap" },
                    "next": "blitz_boss_crush"
                },
                {
                    "id": "blitz_boss_crush",
                    "type": "objective",
                    "title_key": "tutorial.blitz_obj_title",
                    "body_key": "tutorial.blitz_obj_body",
                    "hint_key": "tutorial.blitz_boss_hint",
                    "speaker": "caesar",
                    "trigger": {
                        "type": "defeated",
                        "target": "sector_boss",
                        "scope": "episode"
                    },
                    "marker": { "target": "sector_boss" },
                    "guide": { "kind": "world", "target": "target_action", "gesture": "tap" },
                    "next": "blitz_victory"
                },
                {
                    "id": "blitz_victory",
                    "type": "end",
                    "title_key": "tutorial.blitz_victory_title",
                    "body_key": "tutorial.blitz_victory_body",
                    "speaker": "caesar",
                    "presentation": "chapter"
                }
            ],
            "reactions": [
                {
                    "id": "blitz_boss_panic",
                    "when": { "type": "pre_defeat", "target": "sector_boss" },
                    "during": "blitz_boss_crush",
                    "presentation": "exchange",
                    "exchange": [
                        {
                            "speaker": "sector_boss",
                            "emote": "help",
                            "emote_text_key": "tutorial.blitz_emote_panic"
                        }
                    ]
                }
            ]
        });

        println!(
            "Generated {episode_id} ({}) on {map_name} player=({},{}) boss={boss_troops}",
            arch.label(),
            player_spawn.0,
            player_spawn.1
        );
        if !args.dry_run {
            let roster_out = args.campaign_root.join(format!("{episode_id}.json"));
            let trig_out = args.campaign_root.join(format!("{episode_id}.triggers.json"));
            fs::write(roster_out, serde_json::to_string_pretty(&roster_json)?)?;
            fs::write(trig_out, serde_json::to_string_pretty(&triggers_json)?)?;
        }
    }
    Ok(())
}
