//! Read-only compatibility for recordings made before typed emotes.
//! This is the only owner of the old reaction format and its gameplay effects.
//! See docs/nameplates-and-emotes.md before changing it.
use bincode::Options;
use serde::{Deserialize, Serialize};
use sow_core::game::BuildingKind;
use sow_core::{
    engine::SowEngine,
    protocol::{
        AttackIntent, CampaignRelation, GameplayIntent, RacerControlsIntent, StampedIntent, Turn,
    },
};
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TurnV1 {
    turn_number: u64,
    intents: Vec<StampedV1>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct StampedV1 {
    player_id: u16,
    intent: IntentV1,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
enum IntentV1 {
    Attack(AttackIntent),
    CancelAttack {
        attack_id: u64,
    },
    Spawn {
        x: u32,
        y: u32,
    },
    /// Water transport: raw clicked tile (`y * width + x`); sim snaps to nearest owned shoreline.
    LaunchFleet {
        target_tile: u32,
        troops: Option<f64>,
    },
    RecallFleet {
        fleet_id: u64,
    },
    /// `target_tile` is the clicked tile index (`y * width + x`);
    BuildStructure {
        kind: BuildingKind,
        target_tile: u32,
    },
    BuildShip {
        port_tile: u32,
        kind: sow_core::game::UnitType,
    },
    MoveWarships {
        unit_ids: Vec<u64>,
        target_tile: u32,
    },
    /// pays gold cost again and increments `level`.
    UpgradeStructure {
        building_id: u64,
    },
    /// Informs the engine that the player has disconnected or resigned.
    Resign,
    MarkDisconnected {
        is_disconnected: bool,
    },
    ExpressEmoji {
        emoji: String,
        pinned: bool,
    },
    ProposeAlliance {
        target_player: sow_core::player::PlayerId,
    },
    AcceptAlliance {
        target_player: sow_core::player::PlayerId,
    },
    RejectAlliance {
        target_player: sow_core::player::PlayerId,
    },
    BreakAlliance {
        target_player: sow_core::player::PlayerId,
    },
    SendResources {
        target_player: sow_core::player::PlayerId,
        gold: f64,
        troops: f64,
    },
    RequestResources {
        target_player: sow_core::player::PlayerId,
        gold: f64,
        troops: f64,
    },
    AcceptResourceRequest {
        target_player: sow_core::player::PlayerId,
    },
    RejectResourceRequest {
        target_player: sow_core::player::PlayerId,
    },
    LaunchNuke {
        kind: sow_core::game::NukeKind,
        target_tile: u32,
    },
    /// INK TIDE (Wave-Racer-style) boat controls — lockstep input for the
    /// racer. The relay treats this opaquely: stamps, stores, broadcasts.
    /// Only racer clients decode it. Keep new variants appended after this.
    RacerControls(RacerControlsIntent),
    /// Resolves a single-player campaign contact choice after Rust verifies contact and cost.
    ResolveCampaignDiplomacy {
        target_player: sow_core::player::PlayerId,
        relation: CampaignRelation,
        gold_cost: f64,
    },
}

#[derive(Clone, Debug)]
struct ReactionV1 {
    emoji: String,
    pinned: bool,
}

impl IntentV1 {
    fn into_current(self) -> (GameplayIntent, Option<ReactionV1>) {
        let intent = match self {
            Self::Attack(v) => GameplayIntent::Attack(v),
            Self::CancelAttack { attack_id } => GameplayIntent::CancelAttack { attack_id },
            Self::Spawn { x, y } => GameplayIntent::Spawn { x, y },
            Self::LaunchFleet {
                target_tile,
                troops,
            } => GameplayIntent::LaunchFleet {
                target_tile,
                troops,
            },
            Self::RecallFleet { fleet_id } => GameplayIntent::RecallFleet { fleet_id },
            Self::BuildStructure { kind, target_tile } => {
                GameplayIntent::BuildStructure { kind, target_tile }
            }
            Self::BuildShip { port_tile, kind } => GameplayIntent::BuildShip { port_tile, kind },
            Self::MoveWarships {
                unit_ids,
                target_tile,
            } => GameplayIntent::MoveWarships {
                unit_ids,
                target_tile,
            },
            Self::UpgradeStructure { building_id } => {
                GameplayIntent::UpgradeStructure { building_id }
            }
            Self::Resign => GameplayIntent::Resign,
            Self::MarkDisconnected { is_disconnected } => {
                GameplayIntent::MarkDisconnected { is_disconnected }
            }
            Self::ExpressEmoji { emoji, pinned } => {
                return (
                    GameplayIntent::ClearEmote,
                    Some(ReactionV1 { emoji, pinned }),
                );
            }
            Self::ProposeAlliance { target_player } => {
                GameplayIntent::ProposeAlliance { target_player }
            }
            Self::AcceptAlliance { target_player } => {
                GameplayIntent::AcceptAlliance { target_player }
            }
            Self::RejectAlliance { target_player } => {
                GameplayIntent::RejectAlliance { target_player }
            }
            Self::BreakAlliance { target_player } => {
                GameplayIntent::BreakAlliance { target_player }
            }
            Self::SendResources {
                target_player,
                gold,
                troops,
            } => GameplayIntent::SendResources {
                target_player,
                gold,
                troops,
            },
            Self::RequestResources {
                target_player,
                gold,
                troops,
            } => GameplayIntent::RequestResources {
                target_player,
                gold,
                troops,
            },
            Self::AcceptResourceRequest { target_player } => {
                GameplayIntent::AcceptResourceRequest { target_player }
            }
            Self::RejectResourceRequest { target_player } => {
                GameplayIntent::RejectResourceRequest { target_player }
            }
            Self::LaunchNuke { kind, target_tile } => {
                GameplayIntent::LaunchNuke { kind, target_tile }
            }
            Self::RacerControls(v) => GameplayIntent::RacerControls(v),
            Self::ResolveCampaignDiplomacy {
                target_player,
                relation,
                gold_cost,
            } => GameplayIntent::ResolveCampaignDiplomacy {
                target_player,
                relation,
                gold_cost,
            },
        };
        (intent, None)
    }
}

#[derive(Default)]
struct ExpressionV1 {
    dagger: bool,
    pinned: bool,
    timer: u32,
}
impl ExpressionV1 {
    fn retaliation_ticks(&self) -> u32 {
        if !self.dagger {
            0
        } else if self.pinned {
            u32::MAX
        } else {
            self.timer
        }
    }
}

pub(crate) struct LegacyReplay {
    reactions: HashMap<(usize, usize), ReactionV1>,
    expressions: BTreeMap<u16, ExpressionV1>,
}

/// Metadata is registered by the trusted match orchestrator, never selected by the upload.
pub(crate) fn decode(
    bytes: &[u8],
    version: u64,
    limit: u64,
) -> Result<(Vec<Turn>, Option<LegacyReplay>), String> {
    let options = || {
        bincode::DefaultOptions::new()
            .with_fixint_encoding()
            .with_limit(limit)
            .reject_trailing_bytes()
    };
    match version {
        2 | sow_core::protocol::GAMEPLAY_FORMAT_VERSION => options()
            .deserialize(bytes)
            .map(|turns| (turns, None))
            .map_err(|e| e.to_string()),
        1 => {
            let old: Vec<TurnV1> = options().deserialize(bytes).map_err(|e| e.to_string())?;
            let mut legacy = LegacyReplay {
                reactions: HashMap::new(),
                expressions: BTreeMap::new(),
            };
            let turns = old
                .into_iter()
                .enumerate()
                .map(|(turn_index, turn)| Turn {
                    turn_number: turn.turn_number,
                    intents: turn
                        .intents
                        .into_iter()
                        .enumerate()
                        .map(|(intent_index, stamped)| {
                            let (intent, reaction) = stamped.intent.into_current();
                            if let Some(reaction) = reaction {
                                legacy
                                    .reactions
                                    .insert((turn_index, intent_index), reaction);
                            }
                            StampedIntent {
                                player_id: stamped.player_id,
                                intent,
                            }
                        })
                        .collect(),
                })
                .collect();
            Ok((turns, Some(legacy)))
        }
        other => Err(format!("unsupported replay format {other}")),
    }
}

pub(crate) fn apply_turn(
    engine: &mut SowEngine,
    turn: &Turn,
    index: usize,
    legacy: &mut Option<LegacyReplay>,
) {
    let Some(legacy) = legacy else {
        engine.apply_intents(&turn.intents);
        engine.tick();
        return;
    };
    for (intent_index, stamped) in turn.intents.iter().enumerate() {
        if let Some(reaction) = legacy.reactions.get(&(index, intent_index)) {
            let expression = legacy.expressions.entry(stamped.player_id).or_default();
            expression.dagger = reaction.emoji == "🗡️";
            expression.pinned = reaction.pinned;
            expression.timer = if reaction.pinned {
                0
            } else {
                (3000.0 / engine.state.config.tick_rate_ms).round() as u32
            };
            if let Some(player) = engine.state.player_mut(stamped.player_id) {
                player.betrayal_retaliation_ticks = expression.retaliation_ticks();
            }
            continue;
        }
        // Keep the original index: fleet identifiers depend on it.
        engine.apply_stamped_intent(stamped, intent_index as u32);
        if matches!(stamped.intent, GameplayIntent::BreakAlliance { .. }) {
            let expression = legacy.expressions.entry(stamped.player_id).or_default();
            if let Some(player) = engine.state.player_mut(stamped.player_id) {
                expression.dagger = true;
                expression.timer = player.betrayal_retaliation_ticks;
                player.betrayal_retaliation_ticks = expression.retaliation_ticks();
            }
        }
    }
    let reactions_advance = !matches!(
        engine.state.phase,
        sow_core::game::GamePhase::Spawning { .. }
    );
    engine.tick();
    for (&id, expression) in &mut legacy.expressions {
        let Some(player) = engine.state.player_mut(id) else {
            continue;
        };
        let expected = expression
            .retaliation_ticks()
            .saturating_sub(u32::from(reactions_advance));
        if player.betrayal_retaliation_ticks != expected {
            // A disconnected human can break an alliance from the AI during tick().
            expression.dagger = player.betrayal_retaliation_ticks > 0;
            expression.timer = player
                .betrayal_retaliation_ticks
                .saturating_add(u32::from(expression.pinned && reactions_advance));
        } else if reactions_advance && !expression.pinned {
            expression.timer = expression.timer.saturating_sub(1);
            if expression.timer == 0 {
                expression.dagger = false;
            }
        }
        player.betrayal_retaliation_ticks = expression.retaliation_ticks();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> SowEngine {
        let config = sow_core::game_config::GameConfig::default();
        let mut state = sow_core::game::GameState::new(7, 4, 4, config.clone());
        state.phase = sow_core::game::GamePhase::Playing;
        for id in 1..=2 {
            let mut player =
                sow_core::player::Player::new_human(id, format!("Hero {id}"), [1.0; 3], &config);
            player.has_spawned = true;
            player.alliances.push(3 - id);
            state.register_player(player);
            state.set_tile_owner(u32::from(id), 0, id);
        }
        SowEngine::new(
            state,
            sow_core::water_components::WaterComponents::default(),
        )
    }

    #[test]
    fn v1_pin_and_reaction_replacement_preserve_historical_retaliation() {
        let (turns, mut legacy) =
            decode(include_bytes!("../tests/fixtures/emotes-v1.bin"), 1, 4096).unwrap();
        let mut engine = engine();
        apply_turn(&mut engine, &turns[0], 0, &mut legacy);
        assert!(!sow_core::diplomacy::can_retaliate_against(
            engine.state.player(1).unwrap()
        ));
        apply_turn(&mut engine, &turns[1], 1, &mut legacy);
        let player = engine.state.player(1).unwrap();
        assert!(player.alliances.is_empty());
        assert_eq!(player.betrayal_retaliation_ticks, u32::MAX);
        assert!(player.active_emote.is_none());
        apply_turn(&mut engine, &turns[2], 2, &mut legacy);
        assert!(!sow_core::diplomacy::can_retaliate_against(
            engine.state.player(1).unwrap()
        ));
        assert!(engine.state.player(1).unwrap().traitor);
        // Partial participant closures use the same V1 bytes and frozen discriminants.
        let prefix: Vec<TurnV1> =
            bincode::deserialize(include_bytes!("../tests/fixtures/emotes-v1.bin")).unwrap();
        let partial = bincode::serialize(&prefix[..2]).unwrap();
        assert_eq!(decode(&partial, 1, 4096).unwrap().0, turns[..2]);
    }

    #[test]
    fn frozen_v1_fixture_keeps_reaction_and_diplomacy_indexes() {
        let bytes = include_bytes!("../tests/fixtures/emotes-v1.bin");
        let original: Vec<TurnV1> = bincode::deserialize(bytes).unwrap();
        assert_eq!(bincode::serialize(&original).unwrap(), bytes);
        let (turns, legacy) = decode(bytes, 1, 4096).unwrap();
        assert_eq!(turns.len(), 4);
        assert!(matches!(
            turns[0].intents[0].intent,
            GameplayIntent::ClearEmote
        ));
        assert!(matches!(
            turns[1].intents[0].intent,
            GameplayIntent::BreakAlliance { target_player: 2 }
        ));
        assert!(matches!(turns[3].intents[0].intent, GameplayIntent::Resign));
        let legacy = legacy.unwrap();
        assert!(legacy.reactions[&(0, 0)].pinned);
        assert!(!legacy.reactions[&(2, 0)].pinned);
        assert!(decode(bytes, 99, 4096).is_err());
    }

    #[test]
    fn current_recordings_round_trip_all_emotes_and_clear() {
        let turns = sow_core::emote::EMOTES
            .iter()
            .enumerate()
            .map(|(i, e)| Turn {
                turn_number: i as u64,
                intents: vec![
                    StampedIntent {
                        player_id: 1,
                        intent: GameplayIntent::ExpressEmote {
                            kind: e.kind,
                            pinned: i % 2 == 0,
                        },
                    },
                    StampedIntent {
                        player_id: 1,
                        intent: GameplayIntent::ClearEmote,
                    },
                ],
            })
            .collect::<Vec<_>>();
        let bytes = bincode::serialize(&turns).unwrap();
        let (decoded, legacy) =
            decode(&bytes, sow_core::protocol::GAMEPLAY_FORMAT_VERSION, 4096).unwrap();
        assert_eq!(decoded, turns);
        assert!(legacy.is_none());
        // V3 changes placement rules, not the serialized action layout.
        let (historical, legacy) = decode(&bytes, 2, 4096).unwrap();
        assert_eq!(historical, turns);
        assert!(legacy.is_none());
    }
}
