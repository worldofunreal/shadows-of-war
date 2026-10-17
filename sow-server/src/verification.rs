//! Match-scoped continuation of the canonical replay verifier.
use super::*;
use bincode::Options;
use sow_data::profile::{MatchProgress, MatchStartRecord, VerifiedMatchResult, VerifiedProgress};
use std::{collections::HashMap, io::Write, time::Instant};

const SLICE: Duration = Duration::from_millis(100);
const RESIDENT_BYTES: u64 = 2 * 1024 * 1024 * 1024;

fn storage_root() -> PathBuf {
    env::var_os("SOW_VERIFIER_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            env::var_os("SOW_MAPS_CATALOG_PATH")
                .map(PathBuf::from)
                .and_then(|p| p.parent().map(|p| p.join("verified")))
                .unwrap_or_else(|| PathBuf::from("state/verified"))
        })
}

fn map_path(config: &GameConfig, root: &Path) -> Result<PathBuf, ReplayVerificationError> {
    let key = sow_core::maps::map_key(&config.map_name);
    if key.is_empty() {
        return Err(ReplayVerificationError::MissingMap(
            "trusted map name is invalid".into(),
        ));
    }
    let plain = root.join(&key).join("map.bin");
    Ok(if plain.is_file() {
        plain
    } else {
        root.join(key).join("map.bin.br")
    })
}

pub(super) fn match_map_bytes(
    config: &GameConfig,
    start: &MatchStartRecord,
    root: &Path,
) -> Result<Vec<u8>, ReplayVerificationError> {
    let expected = start
        .metadata
        .get("map_sha256")
        .and_then(serde_json::Value::as_str);
    let cached = expected
        .filter(|hash| hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
        .map(|hash| storage_root().join("maps").join(format!("{hash}.bin")));
    let path = cached
        .filter(|p| p.is_file())
        .unwrap_or(map_path(config, root)?);
    let length = std::fs::metadata(&path)
        .map_err(|e| ReplayVerificationError::MissingMap(e.to_string()))?
        .len();
    if length > 32 * 1024 * 1024 {
        return Err(ReplayVerificationError::MissingMap(
            "map exceeds size limit".into(),
        ));
    }
    let bytes =
        std::fs::read(path).map_err(|e| ReplayVerificationError::MissingMap(e.to_string()))?;
    if expected.is_some_and(|hash| hash != hex::encode(Sha256::digest(&bytes))) {
        return Err(ReplayVerificationError::MissingMap(
            "registered map digest mismatch".into(),
        ));
    }
    Ok(bytes)
}

/// Retain the registered map independently of later asset releases.
pub(super) async fn register_map(config: &GameConfig) -> Result<String, String> {
    let path = map_path(config, &map_catalog::maps_root()).map_err(|e| format!("{e:?}"))?;
    let bytes = tokio::fs::read(path).await.map_err(|e| e.to_string())?;
    if bytes.len() > 32 * 1024 * 1024 {
        return Err("map exceeds size limit".into());
    }
    let hash = hex::encode(Sha256::digest(&bytes));
    let root = storage_root().join("maps");
    tokio::fs::create_dir_all(&root)
        .await
        .map_err(|e| e.to_string())?;
    let path = root.join(format!("{hash}.bin"));
    if !tokio::fs::try_exists(&path)
        .await
        .map_err(|e| e.to_string())?
    {
        use tokio::io::AsyncWriteExt;
        let temporary = path.with_extension(format!("{}.tmp", rand::random::<u64>()));
        let mut file = tokio::fs::File::create(&temporary)
            .await
            .map_err(|e| e.to_string())?;
        file.write_all(&bytes).await.map_err(|e| e.to_string())?;
        file.sync_all().await.map_err(|e| e.to_string())?;
        drop(file);
        tokio::fs::rename(temporary, path)
            .await
            .map_err(|e| e.to_string())?;
        std::fs::File::open(root)
            .and_then(|d| d.sync_all())
            .map_err(|e| e.to_string())?;
    }
    Ok(hash)
}

struct Resident {
    progress: Option<VerifiedPrefix>,
    saved: Instant,
    used: Instant,
    bytes: u64,
}

pub(super) struct Sessions {
    resident: HashMap<String, Resident>,
    root: PathBuf,
    budget: u64,
}

impl Sessions {
    pub(super) fn new() -> Self {
        Self {
            resident: HashMap::new(),
            root: storage_root(),
            budget: RESIDENT_BYTES,
        }
    }

    fn take(&mut self, id: &str) -> Result<Resident, ReplayVerificationError> {
        if let Some(entry) = self.resident.remove(id) {
            return Ok(entry);
        }
        let path = self.root.join(format!("{id}.checkpoint"));
        let progress = match std::fs::File::open(&path) {
            Ok(file) => {
                let decoded = bincode::DefaultOptions::new()
                    .with_fixint_encoding()
                    .with_limit(RESIDENT_BYTES)
                    .deserialize_from(file)
                    .map(|saved: (u64, VerifiedPrefix)| saved);
                match decoded {
                    Ok((epoch, saved)) if epoch == BUILD_EPOCH => Some(saved),
                    Ok(_) => {
                        log::info!(
                            "[rewards] match={id} checkpoint belongs to another simulation build; replaying durable turns"
                        );
                        std::fs::remove_file(&path)
                            .map_err(|e| ReplayVerificationError::Unavailable(e.to_string()))?;
                        None
                    }
                    Err(error) => {
                        log::warn!(
                            "[rewards] match={id} checkpoint is unreadable; rebuilding from saved turns: {error}"
                        );
                        std::fs::remove_file(&path)
                            .map_err(|e| ReplayVerificationError::Unavailable(e.to_string()))?;
                        None
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(ReplayVerificationError::Unavailable(e.to_string())),
        };
        Ok(Resident {
            progress,
            saved: Instant::now(),
            used: Instant::now(),
            bytes: 0,
        })
    }

    fn save(&self, id: &str, entry: &Resident) -> Result<(), ReplayVerificationError> {
        let Some(progress) = &entry.progress else {
            return Ok(());
        };
        let save = || -> Result<(), Box<dyn std::error::Error>> {
            std::fs::create_dir_all(&self.root)?;
            let path = self.root.join(format!("{id}.checkpoint"));
            let temporary = path.with_extension("checkpoint.tmp");
            let mut file = std::io::BufWriter::new(std::fs::File::create(&temporary)?);
            bincode::serialize_into(&mut file, &(BUILD_EPOCH, progress))?;
            file.flush()?;
            file.get_ref().sync_all()?;
            std::fs::rename(temporary, path)?;
            std::fs::File::open(&self.root)?.sync_all()?;
            Ok(())
        };
        let started = Instant::now();
        save()
            .map_err(|e| ReplayVerificationError::Unavailable(format!("checkpoint save: {e}")))?;
        log::info!(
            "[exit-timing] checkpoint match={id} turns={} save_ms={}",
            progress.turns,
            started.elapsed().as_millis()
        );
        Ok(())
    }

    fn put(
        &mut self,
        id: &str,
        mut entry: Resident,
        confirm: bool,
    ) -> Result<(), ReplayVerificationError> {
        // Include the map-sized pathfinding scratch space omitted from storage.
        entry.bytes = entry.progress.as_ref().map_or(0, |p| {
            bincode::serialized_size(p)
                .unwrap_or(RESIDENT_BYTES)
                .saturating_mul(2)
                .saturating_add(p.engine.state.map.terrain.len() as u64 * 32)
        });
        entry.used = Instant::now();
        if confirm || entry.saved.elapsed() >= Duration::from_secs(30) {
            if let Err(error) = self.save(id, &entry) {
                self.resident.insert(id.into(), entry);
                return Err(error);
            }
            entry.saved = Instant::now();
        }
        self.resident.insert(id.into(), entry);
        while self.resident.values().map(|e| e.bytes).sum::<u64>() > self.budget {
            let key = self
                .resident
                .iter()
                .min_by_key(|(_, e)| e.used)
                .map(|(k, _)| k.clone())
                .unwrap();
            self.save(&key, &self.resident[&key])?;
            self.resident.remove(&key);
        }
        Ok(())
    }

    pub(super) fn legacy(
        &mut self,
        id: &str,
        bytes: &[u8],
        start: &MatchStartRecord,
        maps: &Path,
        personal: Option<(String, u64)>,
    ) -> Result<(VerifiedMatchResult, bool), ReplayVerificationError> {
        let version = start
            .metadata
            .get("replay_format")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(1);
        let (turns, legacy) = replay::decode(bytes, version, MAX_REPLAY_BYTES as u64)
            .map_err(ReplayVerificationError::Invalid)?;
        let mut entry = self.take(id)?;
        if entry
            .progress
            .as_ref()
            .is_some_and(|p| p.turns > turns.len())
        {
            if let Some((account, turn)) = &personal {
                if let Some(outcome) = entry
                    .progress
                    .as_ref()
                    .and_then(|p| p.outcomes.get(account))
                    .filter(|o| o.result.personal_closure_turn == Some(*turn))
                {
                    let result = outcome.result.clone();
                    self.put(id, entry, true)?;
                    return Ok((result, true));
                }
            }
            entry.progress = None;
        }
        let result = advance_replay(
            id,
            &turns,
            version,
            legacy,
            start,
            maps,
            personal,
            &mut entry.progress,
            Some(SLICE),
        );
        let complete = result.as_ref().is_ok_and(|(_, done)| *done);
        self.put(id, entry, complete)?;
        result
    }

    pub(super) fn advance(
        &mut self,
        batch: &MatchProgress,
        start: &MatchStartRecord,
        maps: &Path,
    ) -> Result<VerifiedProgress, ReplayVerificationError> {
        if start
            .metadata
            .get("replay_format")
            .and_then(serde_json::Value::as_u64)
            != Some(sow_core::protocol::GAMEPLAY_FORMAT_VERSION)
            || batch.replay_data.len() > sow_data::MAX_MATCH_CHUNK_BYTES
        {
            return Err(ReplayVerificationError::Invalid(
                "invalid progress recording".into(),
            ));
        }
        let (turns, _) = replay::decode(
            &batch.replay_data,
            sow_core::protocol::GAMEPLAY_FORMAT_VERSION,
            sow_data::MAX_MATCH_CHUNK_BYTES as u64,
        )
        .map_err(ReplayVerificationError::Invalid)?;
        if turns
            .first()
            .is_some_and(|t| t.turn_number != batch.first_turn)
        {
            return Err(ReplayVerificationError::Invalid(
                "progress turn identity mismatch".into(),
            ));
        }
        let mut entry = self.take(&batch.match_id)?;
        let previous_outcomes = entry.progress.as_ref().map_or(0, |p| p.outcomes.len());
        let advanced = advance_replay(
            &batch.match_id,
            &turns,
            sow_core::protocol::GAMEPLAY_FORMAT_VERSION,
            None,
            start,
            maps,
            None,
            &mut entry.progress,
            Some(SLICE),
        );
        let (result, complete) = match advanced {
            Ok(result) => result,
            Err(error) => {
                self.put(&batch.match_id, entry, false)?;
                return Err(error);
            }
        };
        let progress = entry.progress.as_mut().unwrap();
        for closure in &batch.participant_closures {
            if let Some(outcome) = progress.outcomes.get_mut(&closure.account_id)
                && outcome.result.personal_closure_turn == Some(closure.turn_number)
            {
                outcome.reason = closure.reason.clone();
            }
        }
        let reply = VerifiedProgress {
            match_id: batch.match_id.clone(),
            sequence: batch.sequence,
            sha256: batch.sha256.clone(),
            complete,
            verified_turns: progress.turns as u64,
            terminal_turn: progress.terminal_turn,
            participants: progress.outcomes.values().cloned().collect(),
            result: (progress.terminal_turn.is_some() || (batch.finished && complete))
                .then_some(result),
        };
        let confirm = progress.outcomes.len() != previous_outcomes || reply.result.is_some();
        self.put(&batch.match_id, entry, confirm)?;
        Ok(reply)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sow_core::protocol::{GameplayIntent, PlayerInfo, StampedIntent, Turn};

    fn fixture(id: &str, root: &Path) -> MatchStartRecord {
        let map = sow_core::map_file::MapFile {
            display_name: "Verifier Arena".into(),
            width: 80,
            height: 40,
            num_land_tiles: 2800,
            spawns: Vec::new(),
            geo_bounds: None,
            default_roster: None,
            rosters: Vec::new(),
            terrain: (0..3200)
                .map(|i| if (35..45).contains(&(i % 80)) { 0 } else { 128 })
                .collect(),
        };
        std::fs::create_dir_all(root.join("verifier_arena")).unwrap();
        std::fs::write(
            root.join("verifier_arena/map.bin"),
            sow_core::map_file::encode(&map),
        )
        .unwrap();
        let config = GameConfig {
            map_name: "Verifier Arena".into(),
            map_width: 80,
            map_height: 40,
            random_spawn: false,
            nation_count: 0,
            bot_count: 0,
            map_control_win_percentage: 0.99,
            ..Default::default()
        };
        let players = (1..=2)
            .map(|id| PlayerInfo {
                id,
                name: format!("Human {id}"),
                player_type: sow_core::player::PlayerType::Human,
                color: [0.5; 3],
                team: None,
                spawn_x: 0,
                spawn_y: 0,
                civilization: sow_core::player::Civilization::Rome,
                leader: sow_core::player::Leader::Caesar,
                skin_style: 0,
                is_ai_controlled: false,
            })
            .collect::<Vec<_>>();
        let accounts = vec![
            "00000000000000000000000000000001".to_string(),
            "00000000000000000000000000000002".to_string(),
        ];
        let roster = players.iter().zip(&accounts).map(|(p,account)|serde_json::json!({"player_id":p.id,"database_account_id":account,"leader":p.leader,"is_internal":false})).collect::<Vec<_>>();
        MatchStartRecord {
            match_id: id.into(),
            player_ids: accounts,
            metadata: serde_json::json!({"seed":20473,"config":config,"players":players,"relay_players":roster,"replay_format":sow_core::protocol::GAMEPLAY_FORMAT_VERSION}),
            ..Default::default()
        }
    }

    fn batch(
        start: &MatchStartRecord,
        sequence: u64,
        turns: &[Turn],
        finished: bool,
    ) -> MatchProgress {
        let mut batch = MatchProgress {
            match_id: start.match_id.clone(),
            sequence,
            first_turn: turns.first().map_or(0, |t| t.turn_number),
            previous_sha256: String::new(),
            sha256: String::new(),
            replay_data: bincode::serialize(turns).unwrap(),
            participant_closures: Vec::new(),
            finished,
        };
        batch.sha256 = hex::encode(Sha256::digest(batch.signing_bytes().unwrap()));
        batch
    }

    fn temporary(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "sow-verifier-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn eliminated_teammate_keeps_observing_until_team_result_is_final() {
        assert_eq!(
            super::participant_closure_reason(false, true, true, false),
            None
        );
        assert_eq!(
            super::participant_closure_reason(false, true, true, true),
            Some("defeated")
        );
        assert_eq!(
            super::participant_closure_reason(false, true, false, false),
            Some("defeated")
        );
        assert_eq!(
            super::participant_closure_reason(true, false, true, false),
            Some("voluntary")
        );
    }

    #[test]
    fn four_hour_active_matches_interleave_spill_restore_and_only_check_the_closing_tail() {
        let root = temporary("four-hours");
        let first = fixture("424250", &root);
        let mut second = first.clone();
        second.match_id = "424251".into();
        let mut sessions = Sessions {
            resident: HashMap::new(),
            root: root.join("checkpoints"),
            budget: RESIDENT_BYTES,
        };
        let count = 144_000u64;
        let turns = (0..count)
            .map(|turn_number| {
                let intents = if turn_number == 0 {
                    vec![
                        StampedIntent {
                            player_id: 1,
                            intent: GameplayIntent::Spawn { x: 15, y: 20 },
                        },
                        StampedIntent {
                            player_id: 2,
                            intent: GameplayIntent::Spawn { x: 65, y: 20 },
                        },
                    ]
                } else if turn_number % 200 == 0 {
                    (1..=2)
                        .map(|player_id| StampedIntent {
                            player_id,
                            intent: GameplayIntent::Attack(sow_core::protocol::AttackIntent {
                                target_owner: 0,
                                troops: Some(20.0),
                            }),
                        })
                        .collect()
                } else {
                    Vec::new()
                };
                Turn {
                    turn_number,
                    intents,
                }
            })
            .collect::<Vec<_>>();
        let started = Instant::now();
        for (sequence, chunk) in turns.chunks(50).enumerate() {
            for start in [&first, &second] {
                let batch = batch(start, sequence as u64, chunk, false);
                loop {
                    let checked = sessions.advance(&batch, start, &root).unwrap();
                    assert!(
                        checked.result.is_none(),
                        "the four-hour simulation must stay active"
                    );
                    assert!(checked.participants.is_empty());
                    if checked.complete {
                        break;
                    }
                }
            }
            if sequence == 100 || sequence == 1500 {
                sessions.budget = 1;
                let entry = sessions.take(&first.match_id).unwrap();
                sessions.put(&first.match_id, entry, true).unwrap();
                assert!(
                    sessions.resident.is_empty(),
                    "both interleaved matches must spill to disk"
                );
                sessions.budget = RESIDENT_BYTES;
            }
        }
        let simulated_ms = started.elapsed().as_millis();
        let mut first_entry = sessions.take(&first.match_id).unwrap();
        assert_eq!(
            first_entry.progress.as_ref().unwrap().engine.state.tick,
            count
        );
        sessions.save(&first.match_id, &first_entry).unwrap();
        first_entry.progress = None;
        drop(first_entry);
        let mut restarted = Sessions {
            resident: HashMap::new(),
            root: sessions.root.clone(),
            budget: RESIDENT_BYTES,
        };
        let close = [Turn {
            turn_number: count,
            intents: vec![StampedIntent {
                player_id: 1,
                intent: GameplayIntent::Resign,
            }],
        }];
        let before = Instant::now();
        let checked = restarted
            .advance(&batch(&first, 2880, &close, true), &first, &root)
            .unwrap();
        let closing_ms = before.elapsed().as_millis();
        assert!(checked.complete);
        assert_eq!(checked.verified_turns, count + 1);
        let personal = checked
            .participants
            .iter()
            .find(|p| p.account_id == first.player_ids[0])
            .unwrap();
        assert_eq!(personal.result.personal_closure_turn, Some(count));
        assert!(
            !personal
                .result
                .participants
                .iter()
                .find(|p| p.player_id == 1)
                .unwrap()
                .won
        );
        assert!(
            checked
                .result
                .as_ref()
                .unwrap()
                .participants
                .iter()
                .find(|p| p.player_id == 2)
                .unwrap()
                .won
        );
        let mut full = turns;
        full.extend(close);
        let cold = super::super::verify_replay(
            &first.match_id,
            &bincode::serialize(&full).unwrap(),
            &first,
            &root,
            None,
            &mut None,
        )
        .unwrap();
        assert_eq!(
            checked.result.unwrap(),
            cold,
            "restarted incremental result differs from full verification"
        );
        let output = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../dist/.sow-state/progressive-verification-benchmark.json");
        std::fs::create_dir_all(output.parent().unwrap()).unwrap();
        std::fs::write(output,serde_json::to_vec_pretty(&serde_json::json!({"active_match_seconds":14400,"concurrent_matches":2,"turns_per_match":count,"closing_turns":1,"simulation_ms":simulated_ms,"closing_restore_ms":closing_ms})).unwrap()).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn natural_finish_ignores_registered_actions_after_game_over() {
        let root = temporary("outcomes");
        let start = fixture("424252", &root);
        let mut sessions = Sessions {
            resident: HashMap::new(),
            root: root.join("checkpoints"),
            budget: RESIDENT_BYTES,
        };
        let turns = (0..300)
            .map(|turn_number| Turn {
                turn_number,
                intents: if turn_number == 0 {
                    vec![
                        StampedIntent {
                            player_id: 1,
                            intent: GameplayIntent::Spawn { x: 15, y: 20 },
                        },
                        StampedIntent {
                            player_id: 2,
                            intent: GameplayIntent::Spawn { x: 65, y: 20 },
                        },
                    ]
                } else {
                    Vec::new()
                },
            })
            .collect::<Vec<_>>();
        let first = batch(&start, 0, &turns, false);
        while !sessions.advance(&first, &start, &root).unwrap().complete {}
        let repeated = sessions.advance(&first, &start, &root).unwrap();
        assert!(
            repeated.result.is_none(),
            "a flush hint cannot claim a game result"
        );
        assert!(repeated.participants.is_empty());
        let last = batch(
            &start,
            1,
            &[Turn {
                turn_number: 300,
                intents: vec![StampedIntent {
                    player_id: 2,
                    intent: GameplayIntent::Resign,
                }],
            }],
            false,
        );
        let finished = sessions.advance(&last, &start, &root).unwrap();
        assert_eq!(
            finished.terminal_turn,
            Some(300),
            "GameOver must be detected without an exit or client hint from the winner"
        );
        assert!(
            finished
                .result
                .as_ref()
                .unwrap()
                .participants
                .iter()
                .find(|p| p.player_id == 1)
                .unwrap()
                .won
        );
        let retry = sessions.advance(&last, &start, &root).unwrap();
        assert_eq!(finished.result, retry.result);

        let trailing = batch(
            &start,
            2,
            &[Turn {
                turn_number: 301,
                intents: vec![StampedIntent {
                    player_id: 1,
                    intent: GameplayIntent::Spawn { x: 20, y: 20 },
                }],
            }],
            false,
        );
        let after_game_over = sessions.advance(&trailing, &start, &root).unwrap();
        assert!(after_game_over.complete);
        assert_eq!(after_game_over.verified_turns, 302);
        assert_eq!(after_game_over.terminal_turn, Some(300));
        assert_eq!(after_game_over.result, finished.result);

        let mut bad = turns;
        bad[100].intents.push(StampedIntent {
            player_id: 1,
            intent: GameplayIntent::Resign,
        });
        assert!(
            sessions
                .advance(&batch(&start, 0, &bad, false), &start, &root)
                .is_err(),
            "contradictory old turns cannot inherit checked state"
        );

        let mut invalid_sender_sessions = Sessions {
            resident: HashMap::new(),
            root: root.join("invalid-sender-checkpoints"),
            budget: RESIDENT_BYTES,
        };
        while !invalid_sender_sessions
            .advance(&first, &start, &root)
            .unwrap()
            .complete
        {}
        invalid_sender_sessions
            .advance(&last, &start, &root)
            .unwrap();
        let unregistered_tail = batch(
            &start,
            2,
            &[Turn {
                turn_number: 301,
                intents: vec![StampedIntent {
                    player_id: 99,
                    intent: GameplayIntent::Spawn { x: 20, y: 20 },
                }],
            }],
            false,
        );
        assert!(invalid_sender_sessions
            .advance(&unregistered_tail, &start, &root)
            .is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}

pub(super) async fn handle_progress(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> axum::response::Response {
    use axum::{Json, http::StatusCode, response::IntoResponse};
    if !admin_request_authorized(&headers, &state.db_secret) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let batch: MatchProgress = match bincode::deserialize(&body) {
        Ok(batch) => batch,
        Err(binary_error) => match serde_json::from_slice(&body) {
            Ok(batch) => batch,
            Err(json_error) => {
                log::warn!(
                    "[rewards] invalid progress request (binary: {binary_error}; JSON: {json_error})"
                );
                return StatusCode::BAD_REQUEST.into_response();
            }
        },
    };
    if batch.match_id.parse::<u64>().is_err() {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let permit = match state.replay_verification.clone().try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };
    let start = state
        .replay_http
        .get(format!(
            "{}/internal/match-start/{}",
            state.db_url.trim_end_matches('/'),
            batch.match_id
        ))
        .bearer_auth(&state.db_secret)
        .send()
        .await;
    let start = match start {
        Ok(r) if r.status().is_success() => r.json::<MatchStartRecord>().await.ok(),
        _ => None,
    };
    let Some(start) = start.filter(|s| s.match_id == batch.match_id) else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    match tokio::task::spawn_blocking(move || {
        let _permit = permit;
        state
            .verified_prefix
            .lock()
            .map_err(|_| {
                ReplayVerificationError::Unavailable("verification state lock failed".into())
            })?
            .advance(&batch, &start, &state.maps_root)
    })
    .await
    {
        Ok(Ok(reply)) => (StatusCode::OK, Json(reply)).into_response(),
        Ok(Err(error)) => {
            let missing = matches!(&error, ReplayVerificationError::Unavailable(reason) if reason == "verified history required");
            let invalid = matches!(&error, ReplayVerificationError::Invalid(_));
            log::warn!("[rewards] progress verification pending: {error:?}");
            (
                if missing {
                    StatusCode::CONFLICT
                } else if invalid {
                    StatusCode::UNPROCESSABLE_ENTITY
                } else {
                    StatusCode::SERVICE_UNAVAILABLE
                },
                Json(serde_json::json!({"error":format!("{error:?}")})),
            )
                .into_response()
        }
        Err(error) => {
            log::error!("progress verifier task: {error}");
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
    }
}

pub(super) async fn handle_result(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    axum::Json(command): axum::Json<sow_data::profile::MatchResultControl>,
) -> axum::response::Response {
    use axum::{Json, http::StatusCode, response::IntoResponse};
    if !admin_request_authorized(&headers, &state.db_secret) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if command.match_id.parse::<u64>().is_err() {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let result = async {
        let start = state
            .replay_http
            .get(format!(
                "{}/internal/match-start/{}",
                state.db_url.trim_end_matches('/'),
                command.match_id
            ))
            .bearer_auth(&state.db_secret)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .json::<MatchStartRecord>()
            .await
            .map_err(|e| e.to_string())?;
        let workers = relay_workers()?;
        let owner = start
            .metadata
            .get("relay_worker_index")
            .and_then(serde_json::Value::as_u64)
            .ok_or("registered relay owner missing")?;
        let worker = workers
            .iter()
            .find(|w| w.id as u64 == owner)
            .ok_or("registered relay owner unavailable")?;
        let path = "/internal/match/result";
        let body = serde_json::to_vec(&command).map_err(|e| e.to_string())?;
        let secret = env::var("SOW_RELAY_CONTROL_SECRET").map_err(|e| e.to_string())?;
        let (timestamp, nonce, signature) = relay_mgmt_headers(&secret, "POST", path, &body)?;
        let url = format!("{}{path}", worker.mgmt_url.trim_end_matches('/'));
        relay_mgmt_client(&url)?
            .post(&url)
            .timeout(Duration::from_secs(3))
            .header("Content-Type", "application/json")
            .header("X-SOW-Timestamp", timestamp)
            .header("X-SOW-Nonce", nonce)
            .header("X-SOW-Signature", signature)
            .body(body)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?;
        Ok::<_, String>(())
    }
    .await;
    match result {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"match_id":command.match_id})),
        )
            .into_response(),
        Err(error) => {
            log::warn!(
                "verified result delivery match={}: {error}",
                command.match_id
            );
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error":error})),
            )
                .into_response()
        }
    }
}
