//! Decodes and replays turn recordings for the server verifier. Only the
//! current `GAMEPLAY_FORMAT_VERSION` is accepted: recordings from earlier
//! versions used other spawn rules and cannot be replayed faithfully, and a
//! deploy restarts the server, so no match spans two formats.
use bincode::Options;
use sow_core::{engine::SowEngine, protocol::Turn};

pub(crate) fn decode(bytes: &[u8], version: u64, limit: u64) -> Result<Vec<Turn>, String> {
    if version != sow_core::protocol::GAMEPLAY_FORMAT_VERSION {
        return Err(format!(
            "recording format {version} is from an earlier game version (current {})",
            sow_core::protocol::GAMEPLAY_FORMAT_VERSION
        ));
    }
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_limit(limit)
        .reject_trailing_bytes()
        .deserialize(bytes)
        .map_err(|e| e.to_string())
}

pub(crate) fn apply_turn(engine: &mut SowEngine, turn: &Turn) {
    engine.apply_intents(&turn.intents);
    engine.tick();
}

#[cfg(test)]
mod tests {
    use super::*;
    use sow_core::protocol::{GameplayIntent, StampedIntent};

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
        let decoded = decode(&bytes, sow_core::protocol::GAMEPLAY_FORMAT_VERSION, 4096).unwrap();
        assert_eq!(decoded, turns);
        for earlier in 1..sow_core::protocol::GAMEPLAY_FORMAT_VERSION {
            assert!(decode(&bytes, earlier, 4096).is_err(), "format {earlier}");
        }
    }
}
