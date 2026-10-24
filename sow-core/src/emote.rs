//! Fixed player communication. See docs/nameplates-and-emotes.md.

use serde::{Deserialize, Serialize};

pub const EMOTE_DURATION_SECS: f32 = 3.0;

/// Order matches the four cells of every hero sheet, read left-to-right.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[repr(usize)]
pub enum AvatarExpression {
    Happy,
    Calm,
    Fear,
    Angry,
}

impl AvatarExpression {
    pub const ALL: [Self; 4] = [Self::Happy, Self::Calm, Self::Fear, Self::Angry];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(usize)]
pub enum EmoteKind {
    Greet,
    Help,
    Threat,
    Celebrate,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct EmoteDefinition {
    pub kind: EmoteKind,
    pub expression: AvatarExpression,
    pub menu_key: &'static str,
    pub bubble_key: &'static str,
    pub notice_key: &'static str,
    #[serde(skip)]
    pub fallback: &'static str,
}

pub const EMOTES: [EmoteDefinition; 4] = [
    EmoteDefinition {
        kind: EmoteKind::Greet,
        expression: AvatarExpression::Calm,
        menu_key: "hud.emote_greet",
        bubble_key: "hud.emote_greet_message",
        notice_key: "hud.emote_greet_notice",
        fallback: "Greetings!",
    },
    EmoteDefinition {
        kind: EmoteKind::Help,
        expression: AvatarExpression::Fear,
        menu_key: "hud.emote_help",
        bubble_key: "hud.emote_help_message",
        notice_key: "hud.emote_help_notice",
        fallback: "Help!",
    },
    EmoteDefinition {
        kind: EmoteKind::Threat,
        expression: AvatarExpression::Angry,
        menu_key: "hud.emote_threat",
        bubble_key: "hud.emote_threat_message",
        notice_key: "hud.emote_threat_notice",
        fallback: "Prepare yourself!",
    },
    EmoteDefinition {
        kind: EmoteKind::Celebrate,
        expression: AvatarExpression::Happy,
        menu_key: "hud.emote_celebrate",
        bubble_key: "hud.emote_celebrate_message",
        notice_key: "hud.emote_celebrate_notice",
        fallback: "Victory!",
    },
];

impl EmoteKind {
    pub const fn definition(self) -> &'static EmoteDefinition {
        &EMOTES[self as usize]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveEmote {
    pub kind: EmoteKind,
    pub pinned: bool,
    pub started_tick: u64,
    pub event_id: u64,
}

impl ActiveEmote {
    pub fn age_seconds(self, tick: u64, tick_rate_ms: f32) -> f32 {
        tick.saturating_sub(self.started_tick) as f32 * tick_rate_ms / 1000.0
    }

    pub fn expired(self, tick: u64, tick_rate_ms: f32) -> bool {
        !self.pinned && self.age_seconds(tick, tick_rate_ms) >= EMOTE_DURATION_SECS
    }
}

/// Starts `kind` over `player`, replacing any current emote; `None` clears it.
pub fn express(player: &mut crate::player::Player, kind: Option<EmoteKind>, pinned: bool, tick: u64) {
    player.active_emote = match kind {
        Some(kind) => {
            player.emote_event_id = player.emote_event_id.wrapping_add(1);
            Some(ActiveEmote {
                kind,
                pinned,
                started_tick: tick,
                event_id: player.emote_event_id,
            })
        }
        None => None,
    };
}

pub fn can_express(player: &crate::player::Player, phase: &crate::game::GamePhase) -> bool {
    matches!(phase, crate::game::GamePhase::Playing)
        && player.player_type == crate::player::PlayerType::Human
        && player.alive
        && player.has_spawned
        && !player.is_ai_controlled
}

/// The same audience is used by map bubbles and HUD notices; camera visibility is separate.
pub fn is_recipient(players: &[crate::protocol::PlayerSnapshot], sender: u16, viewer: u16) -> bool {
    if viewer == 0 {
        return false;
    }
    if sender == viewer {
        return true;
    }
    let Some(me) = players.iter().find(|p| p.id == viewer) else {
        return false;
    };
    let Some(other) = players.iter().find(|p| p.id == sender) else {
        return false;
    };
    me.alliances.contains(&sender) || (me.team.is_some() && me.team == other.team)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> crate::engine::SowEngine {
        let config = crate::game_config::GameConfig::default();
        let mut state = crate::game::GameState::new(7, 4, 4, config.clone());
        state.phase = crate::game::GamePhase::Playing;
        let mut player = crate::player::Player::new_human(1, "Hero".into(), [1.0; 3], &config);
        player.has_spawned = true;
        state.register_player(player);
        crate::engine::SowEngine::new(state, crate::water_components::WaterComponents::default())
    }

    #[test]
    fn only_live_spawned_humans_can_emit_and_unknown_codes_are_rejected() {
        let mut engine = engine();
        for condition in 0..6 {
            let phase = if condition == 4 {
                crate::game::GamePhase::Spawning { end_tick: 30 }
            } else {
                crate::game::GamePhase::Playing
            };
            engine.state.phase = phase;
            let player = engine.state.player_mut(1).unwrap();
            player.alive = condition != 0;
            player.has_spawned = condition != 1;
            player.is_ai_controlled = condition == 2;
            player.player_type = if condition == 3 {
                crate::player::PlayerType::Bot
            } else {
                crate::player::PlayerType::Human
            };
            engine.apply_intents(&[crate::protocol::StampedIntent {
                player_id: 1,
                intent: crate::protocol::GameplayIntent::ExpressEmote {
                    kind: EmoteKind::Help,
                    pinned: true,
                },
            }]);
            assert_eq!(
                engine.state.player(1).unwrap().active_emote.is_some(),
                condition == 5
            );
        }
        assert!(serde_json::from_str::<EmoteKind>("\"custom text\"").is_err());
    }

    #[test]
    fn repetition_pin_clear_and_betrayal_have_independent_owners() {
        let mut engine = engine();
        for definition in EMOTES {
            engine.apply_intents(&[crate::protocol::StampedIntent {
                player_id: 1,
                intent: crate::protocol::GameplayIntent::ExpressEmote {
                    kind: definition.kind,
                    pinned: true,
                },
            }]);
            let first = engine.state.player(1).unwrap().active_emote.unwrap();
            engine.apply_intents(&[crate::protocol::StampedIntent {
                player_id: 1,
                intent: crate::protocol::GameplayIntent::ExpressEmote {
                    kind: definition.kind,
                    pinned: true,
                },
            }]);
            assert!(
                engine
                    .state
                    .player(1)
                    .unwrap()
                    .active_emote
                    .unwrap()
                    .event_id
                    > first.event_id
            );
            engine.apply_intents(&[crate::protocol::StampedIntent {
                player_id: 1,
                intent: crate::protocol::GameplayIntent::BreakAlliance { target_player: 2 },
            }]);
            let player = engine.state.player(1).unwrap();
            assert_eq!(
                player.betrayal_retaliation_ticks,
                crate::diplomacy::HUMAN_BETRAYAL_RETALIATION_TICKS
            );
            assert_eq!(player.active_emote.unwrap().kind, definition.kind);
            engine.apply_intents(&[crate::protocol::StampedIntent {
                player_id: 1,
                intent: crate::protocol::GameplayIntent::ClearEmote,
            }]);
            assert!(engine.state.player(1).unwrap().active_emote.is_none());
            assert!(crate::diplomacy::can_retaliate_against(
                engine.state.player(1).unwrap()
            ));
        }
    }

    #[test]
    fn definitions_are_exhaustive_and_duration_does_not_extend_pinned_bubbles() {
        for (index, definition) in EMOTES.iter().enumerate() {
            assert_eq!(definition.kind as usize, index);
            assert_eq!(
                definition.kind.definition().expression,
                definition.expression
            );
            let mut event = ActiveEmote {
                kind: definition.kind,
                pinned: false,
                started_tick: 5,
                event_id: 1,
            };
            assert!(!event.expired(34, 100.0));
            assert!(event.expired(35, 100.0));
            event.pinned = true;
            assert!(!event.expired(1000, 100.0));
            assert!(event.age_seconds(1000, 100.0) > EMOTE_DURATION_SECS);
        }
    }

    #[test]
    fn live_wire_identity_and_message_audience_are_explicit() {
        let current = crate::protocol::gameplay_build_identity("0.1.3");
        assert_eq!(
            current,
            format!("0.1.3/turns-5/net-5/rules-{}", crate::SIMULATION_RULES_ID)
        );
        assert_ne!(current, "0.1.3");
        let mut engine = engine();
        let config = engine.state.config.clone();
        for id in 2..=4 {
            let mut player =
                crate::player::Player::new_human(id, format!("Hero {id}"), [1.0; 3], &config);
            player.has_spawned = true;
            engine.state.register_player(player);
        }
        engine.state.player_mut(1).unwrap().alliances.push(2);
        engine.state.player_mut(1).unwrap().team = Some(crate::protocol::Team::Red);
        engine.state.player_mut(3).unwrap().team = Some(crate::protocol::Team::Red);
        let snapshot = engine.build_snapshot();
        assert!(is_recipient(&snapshot.players, 1, 1));
        assert!(is_recipient(&snapshot.players, 2, 1));
        assert!(is_recipient(&snapshot.players, 3, 1));
        assert!(!is_recipient(&snapshot.players, 4, 1));
        assert!(!is_recipient(&snapshot.players, 1, 0));
    }
}
