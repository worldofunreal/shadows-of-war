//! Match-scoped continuation of the canonical replay verifier.
use super::*;
use bincode::Options;
use sow_data::profile::{MatchProgress, MatchStartRecord, VerifiedProgress};
use std::{
    collections::HashMap,
    io::Write,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

const SLICE: Duration = Duration::from_millis(100);
const RESIDENT_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const CHECKPOINT_FORMAT: u32 = 4;

fn allocated_bytes<T>(values: &Vec<T>) -> u64 {
    values.capacity().saturating_mul(std::mem::size_of::<T>()) as u64
}

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
    snapshot_bytes: u64,
    checkpoint_required: bool,
    restored_previous: bool,
}

pub(super) struct Sessions {
    resident: Mutex<HashMap<String, Arc<Mutex<Resident>>>>,
    root: PathBuf,
    budget: AtomicU64,
    bytes: AtomicU64,
}

impl Sessions {
    pub(super) fn new() -> Self {
        Self::at(storage_root(), RESIDENT_BYTES)
    }

    fn at(root: PathBuf, budget: u64) -> Self {
        Self {
            resident: Mutex::new(HashMap::new()),
            root,
            budget: AtomicU64::new(budget),
            bytes: AtomicU64::new(0),
        }
    }

    fn slot(&self, id: &str) -> Result<Arc<Mutex<Resident>>, ReplayVerificationError> {
        let mut resident = self.resident.lock().map_err(|_| {
            ReplayVerificationError::Unavailable("match registry lock failed".into())
        })?;
        Ok(resident
            .entry(id.into())
            .or_insert_with(|| {
                Arc::new(Mutex::new(Resident {
                    progress: None,
                    saved: Instant::now(),
                    used: Instant::now(),
                    bytes: 0,
                    snapshot_bytes: 0,
                    checkpoint_required: false,
                    restored_previous: false,
                }))
            })
            .clone())
    }

    fn load(&self, id: &str, entry: &mut Resident) -> Result<(), ReplayVerificationError> {
        if entry.progress.is_some() {
            return Ok(());
        }
        let mut errors = Vec::new();
        for suffix in ["checkpoint", "checkpoint.previous"] {
            let path = self.root.join(format!("{id}.{suffix}"));
            let file = match std::fs::File::open(&path) {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(ReplayVerificationError::Unavailable(error.to_string())),
            };
            let mut reader = std::io::BufReader::new(file);
            let format = bincode::DefaultOptions::new()
                .with_fixint_encoding()
                .with_limit(4)
                .deserialize_from::<_, u32>(&mut reader);
            let format = match format {
                Ok(format) => format,
                Err(error) => {
                    errors.push(format!("checkpoint format is unreadable: {error}"));
                    continue;
                }
            };
            let rules = bincode::DefaultOptions::new()
                .with_fixint_encoding()
                .with_limit(128)
                .deserialize_from::<_, String>(&mut reader);
            match rules {
                Ok(rules) if rules == sow_core::SIMULATION_RULES_ID => {}
                Ok(_) => {
                    errors.push("checkpoint requires its original simulation worker".to_string());
                    continue;
                }
                Err(error) => {
                    errors.push(format!("checkpoint identity is unreadable: {error}"));
                    continue;
                }
            }
            let decoded = match format {
                CHECKPOINT_FORMAT => bincode::DefaultOptions::new()
                    .with_fixint_encoding()
                    .with_limit(RESIDENT_BYTES)
                    .deserialize_from::<_, VerifiedPrefix>(&mut reader)
                    .map_err(|error| error.to_string()),
                _ => {
                    errors.push(format!("unsupported checkpoint format {format}"));
                    continue;
                }
            };
            match decoded {
                Ok(progress) => {
                    entry.progress = Some(progress);
                    entry.restored_previous = suffix == "checkpoint.previous";
                    if !errors.is_empty() {
                        log::warn!(
                            "[rewards] match={id} restored previous checkpoint: {}",
                            errors.join("; ")
                        );
                    }
                    return Ok(());
                }
                Err(error) => errors.push(format!("checkpoint state is unreadable: {error}")),
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(ReplayVerificationError::Unavailable(errors.join("; ")))
        }
    }

    fn save(&self, id: &str, entry: &Resident) -> Result<u64, ReplayVerificationError> {
        let Some(progress) = &entry.progress else {
            return Ok(0);
        };
        let save = || -> Result<u64, Box<dyn std::error::Error>> {
            std::fs::create_dir_all(&self.root)?;
            let path = self.root.join(format!("{id}.checkpoint"));
            let temporary = path.with_extension("checkpoint.tmp");
            let mut file = std::io::BufWriter::new(std::fs::File::create(&temporary)?);
            bincode::serialize_into(
                &mut file,
                &(CHECKPOINT_FORMAT, sow_core::SIMULATION_RULES_ID, progress),
            )?;
            file.flush()?;
            file.get_ref().sync_all()?;
            let bytes = file.get_ref().metadata()?.len();
            drop(file);
            if path.is_file() && !entry.restored_previous {
                std::fs::rename(&path, path.with_extension("checkpoint.previous"))?;
            }
            std::fs::rename(temporary, path)?;
            std::fs::File::open(&self.root)?.sync_all()?;
            Ok(bytes)
        };
        let started = Instant::now();
        let bytes = save()
            .map_err(|e| ReplayVerificationError::Unavailable(format!("checkpoint save: {e}")))?;
        log::info!(
            "[exit-timing] checkpoint match={id} turns={} save_ms={}",
            progress.turns,
            started.elapsed().as_millis()
        );
        Ok(bytes)
    }

    fn put(
        &self,
        id: &str,
        entry: &mut Resident,
        confirm: bool,
    ) -> Result<(), ReplayVerificationError> {
        entry.checkpoint_required |= confirm;
        if entry.checkpoint_required || entry.saved.elapsed() >= Duration::from_secs(30) {
            entry.snapshot_bytes = self.save(id, entry)?;
            entry.saved = Instant::now();
            entry.checkpoint_required = false;
            entry.restored_previous = false;
        } else if entry.snapshot_bytes == 0 {
            entry.snapshot_bytes = entry
                .progress
                .as_ref()
                .map_or(0, |p| bincode::serialized_size(p).unwrap_or(RESIDENT_BYTES));
        }
        // Account for growth without walking and serializing the entire world
        // on every delivery. The periodic snapshot refreshes the baseline.
        let bytes = entry.progress.as_ref().map_or(0, |p| {
            let engine = &p.engine;
            entry
                .snapshot_bytes
                .saturating_mul(2)
                .saturating_add(engine.state.map.terrain.len() as u64 * 32)
                .saturating_add(allocated_bytes(&engine.attacks))
                .saturating_add(allocated_bytes(&engine.fleets))
                .saturating_add(allocated_bytes(&engine.buildings))
                .saturating_add(allocated_bytes(&engine.projectiles))
                .saturating_add(allocated_bytes(&engine.state.players))
        });
        if bytes >= entry.bytes {
            self.bytes.fetch_add(bytes - entry.bytes, Ordering::Relaxed);
        } else {
            self.bytes.fetch_sub(entry.bytes - bytes, Ordering::Relaxed);
        }
        entry.bytes = bytes;
        entry.used = Instant::now();
        Ok(())
    }

    fn trim(&self) -> Result<(), ReplayVerificationError> {
        if self.bytes.load(Ordering::Relaxed) <= self.budget.load(Ordering::Relaxed) {
            return Ok(());
        }
        let mut candidates = {
            let registry = self.resident.lock().map_err(|_| {
                ReplayVerificationError::Unavailable("match registry lock failed".into())
            })?;
            registry
                .iter()
                .filter_map(|(id, slot)| {
                    let entry = slot.try_lock().ok()?;
                    (entry.bytes > 0).then(|| (entry.used, id.clone(), slot.clone()))
                })
                .collect::<Vec<_>>()
        };
        candidates.sort_by_key(|(used, _, _)| *used);
        for (_, id, slot) in &candidates {
            if self.bytes.load(Ordering::Relaxed) <= self.budget.load(Ordering::Relaxed) {
                break;
            }
            let Ok(mut entry) = slot.try_lock() else {
                continue;
            };
            if entry.progress.is_none() {
                continue;
            }
            self.save(id, &entry)?;
            self.bytes.fetch_sub(entry.bytes, Ordering::Relaxed);
            entry.bytes = 0;
            entry.snapshot_bytes = 0;
            entry.progress = None;
            entry.restored_previous = false;
        }
        drop(candidates);
        let mut registry = self.resident.lock().map_err(|_| {
            ReplayVerificationError::Unavailable("match registry lock failed".into())
        })?;
        registry.retain(|_, slot| {
            Arc::strong_count(slot) > 1
                || slot
                    .try_lock()
                    .map_or(true, |entry| entry.progress.is_some())
        });
        Ok(())
    }

    pub(super) fn advance(
        &self,
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
        let turns = replay::decode(
            &batch.replay_data,
            sow_core::protocol::GAMEPLAY_FORMAT_VERSION,
            sow_data::MAX_MATCH_CHUNK_BYTES as u64,
        )
        .map_err(ReplayVerificationError::Invalid)?;
        let digest =
            hex::encode(Sha256::digest(batch.signing_bytes().map_err(|error| {
                ReplayVerificationError::Invalid(error.to_string())
            })?));
        if digest != batch.sha256 {
            return Err(ReplayVerificationError::Invalid(
                "progress digest mismatch".into(),
            ));
        }
        if turns
            .first()
            .is_some_and(|t| t.turn_number != batch.first_turn)
        {
            return Err(ReplayVerificationError::Invalid(
                "progress turn identity mismatch".into(),
            ));
        }
        let turn_count = u64::try_from(turns.len())
            .map_err(|_| ReplayVerificationError::Invalid("progress turn count overflow".into()))?;
        let end_turn = batch.first_turn.checked_add(turn_count).ok_or_else(|| {
            ReplayVerificationError::Invalid("progress turn sequence overflow".into())
        })?;
        let slot = self.slot(&batch.match_id)?;
        let mut entry = slot
            .lock()
            .map_err(|_| ReplayVerificationError::Unavailable("match state lock failed".into()))?;
        self.load(&batch.match_id, &mut entry)?;
        let recovery = {
            let saved = entry.progress.as_ref();
            let verified_turns = saved.map_or(0, |progress| progress.turns as u64);
            if let Some(saved) = saved {
                if let Some(active) = &saved.active_progress {
                    if let Some(cursor) = &saved.progress_cursor
                        && (active.sequence != cursor.sequence.saturating_add(1)
                            || active.previous_sha256 != cursor.sha256)
                    {
                        return Err(ReplayVerificationError::Unavailable(
                            "saved progress cursor is inconsistent".into(),
                        ));
                    }
                    if batch.sequence != active.sequence {
                        if batch.sequence > active.sequence {
                            Some((active.sequence, verified_turns))
                        } else {
                            return Err(ReplayVerificationError::Invalid(
                                "stale progress sequence".into(),
                            ));
                        }
                    } else if batch.first_turn != active.first_turn
                        || end_turn != active.end_turn
                        || batch.previous_sha256 != active.previous_sha256
                        || batch.sha256 != active.sha256
                    {
                        return Err(ReplayVerificationError::Invalid(
                            "in-progress delivery conflicts with saved turn stream".into(),
                        ));
                    } else {
                        None
                    }
                } else if let Some(cursor) = &saved.progress_cursor {
                    if batch.sequence == cursor.sequence {
                        if batch.sha256 != cursor.sha256 {
                            return Err(ReplayVerificationError::Invalid(
                                "completed progress sequence conflicts with verified history"
                                    .into(),
                            ));
                        }
                        let reply = saved.last_progress_reply.as_ref().ok_or_else(|| {
                            ReplayVerificationError::Unavailable(
                                "completed progress reply missing from checkpoint".into(),
                            )
                        })?;
                        return Ok(reply.clone());
                    }
                    if batch.sequence < cursor.sequence {
                        return Err(ReplayVerificationError::Invalid(
                            "stale progress sequence".into(),
                        ));
                    }
                    let expected = cursor.sequence.saturating_add(1);
                    if batch.sequence != expected {
                        Some((expected, verified_turns))
                    } else if batch.previous_sha256 != cursor.sha256 {
                        return Err(ReplayVerificationError::Invalid(
                            "progress hash chain mismatch".into(),
                        ));
                    } else if batch.first_turn != cursor.next_turn
                        || verified_turns != cursor.next_turn
                    {
                        return Err(ReplayVerificationError::Invalid(
                            "progress turn cursor mismatch".into(),
                        ));
                    } else {
                        None
                    }
                } else if saved.turns == 0 {
                    if batch.sequence != 0 {
                        Some((0, 0))
                    } else if batch.first_turn != 0 || !batch.previous_sha256.is_empty() {
                        return Err(ReplayVerificationError::Invalid(
                            "turn stream must start at turn zero".into(),
                        ));
                    } else {
                        None
                    }
                } else {
                    return Err(ReplayVerificationError::Unavailable(
                        "saved progress has no current batch cursor".into(),
                    ));
                }
            } else if batch.sequence != 0 {
                Some((0, 0))
            } else if batch.first_turn != 0 || !batch.previous_sha256.is_empty() {
                return Err(ReplayVerificationError::Invalid(
                    "turn stream must start at turn zero".into(),
                ));
            } else {
                None
            }
        };
        if let Some((expected_sequence, verified_turns)) = recovery {
            self.put(&batch.match_id, &mut entry, false)?;
            return Err(ReplayVerificationError::HistoryRequired {
                expected_sequence,
                verified_turns,
            });
        }
        let previous_outcomes = entry.progress.as_ref().map_or(0, |p| p.outcomes.len());
        let previous_terminal = entry.progress.as_ref().and_then(|p| p.terminal_turn);
        let previous_turns = entry.progress.as_ref().map_or(0, |p| p.turns);
        let advanced = advance_replay(
            &batch.match_id,
            &turns,
            start,
            maps,
            &mut entry.progress,
            Some(SLICE),
        );
        let (result, complete) = match advanced {
            Ok(result) => result,
            Err(error) => {
                self.put(&batch.match_id, &mut entry, false)?;
                return Err(error);
            }
        };
        let progress = entry.progress.as_mut().unwrap();
        if progress.active_progress.is_none() {
            progress.active_progress = Some(ActiveProgress {
                sequence: batch.sequence,
                first_turn: batch.first_turn,
                end_turn,
                previous_sha256: batch.previous_sha256.clone(),
                sha256: batch.sha256.clone(),
            });
        }
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
            participants: progress
                .outcomes
                .values()
                .filter(|outcome| {
                    outcome.result.personal_closure_turn.is_some_and(|turn| {
                        turn >= batch.first_turn
                            && turns.last().is_some_and(|last| turn <= last.turn_number)
                    })
                })
                .cloned()
                .collect(),
            result: (progress.terminal_turn.is_some_and(|turn| {
                turn >= batch.first_turn
                    && turns.last().is_some_and(|last| turn <= last.turn_number)
            }) || (batch.finished && complete && progress.terminal_turn.is_none()))
            .then_some(result),
        };
        if complete {
            progress.progress_cursor = Some(ProgressCursor {
                sequence: batch.sequence,
                sha256: batch.sha256.clone(),
                next_turn: end_turn,
            });
            progress.active_progress = None;
            progress.last_progress_reply = Some(reply.clone());
        }
        let confirm = progress.outcomes.len() != previous_outcomes
            || progress.terminal_turn != previous_terminal
            || (batch.finished && complete && progress.turns != previous_turns);
        self.put(&batch.match_id, &mut entry, confirm)?;
        drop(entry);
        drop(slot);
        self.trim()?;
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
                leader: sow_core::player::Leader::of("caesar"),
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
            metadata: serde_json::json!({"seed":20473,"config":config,"players":players,"relay_players":roster,
                "replay_format":sow_core::protocol::GAMEPLAY_FORMAT_VERSION,
                "simulation_rules_id":sow_core::SIMULATION_RULES_ID}),
            ..Default::default()
        }
    }

    fn batch(
        start: &MatchStartRecord,
        sequence: u64,
        turns: &[Turn],
        finished: bool,
    ) -> MatchProgress {
        batch_with_previous(start, sequence, turns, finished, String::new())
    }

    fn batch_with_previous(
        start: &MatchStartRecord,
        sequence: u64,
        turns: &[Turn],
        finished: bool,
        previous_sha256: String,
    ) -> MatchProgress {
        let mut batch = MatchProgress {
            match_id: start.match_id.clone(),
            sequence,
            first_turn: turns.first().map_or(0, |t| t.turn_number),
            previous_sha256,
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
    fn a_busy_match_does_not_lock_another_match() {
        let root = temporary("parallel");
        let first = fixture("424260", &root);
        let mut second = first.clone();
        second.match_id = "424261".into();
        let sessions = Sessions::at(root.join("checkpoints"), RESIDENT_BYTES);
        let first_slot = sessions.slot(&first.match_id).unwrap();
        let busy = first_slot.lock().unwrap();
        let (done, completed) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            scope.spawn(|| {
                let batch = batch(
                    &second,
                    0,
                    &[Turn {
                        turn_number: 0,
                        intents: Vec::new(),
                    }],
                    false,
                );
                done.send(sessions.advance(&batch, &second, &root).is_ok())
                    .unwrap();
            });
            assert!(
                completed.recv_timeout(Duration::from_secs(2)).unwrap(),
                "an unrelated match must advance while the first match is busy"
            );
        });
        drop(busy);
        drop(first_slot);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn progress_recovery_requests_only_the_missing_sequence_and_retries_are_idempotent() {
        let root = temporary("progress-cursor");
        let start = fixture("424262", &root);
        let sessions = Sessions::at(root.join("checkpoints"), RESIDENT_BYTES);
        let turns = [
            Turn {
                turn_number: 0,
                intents: Vec::new(),
            },
            Turn {
                turn_number: 1,
                intents: Vec::new(),
            },
            Turn {
                turn_number: 2,
                intents: Vec::new(),
            },
        ];
        let first = batch(&start, 0, &turns[..1], false);
        let second = batch_with_previous(&start, 1, &turns[1..2], false, first.sha256.clone());
        let third = batch_with_previous(&start, 2, &turns[2..], false, second.sha256.clone());
        let mut tampered = first.clone();
        tampered.sha256 = "f".repeat(64);
        assert!(matches!(
            sessions.advance(&tampered, &start, &root),
            Err(ReplayVerificationError::Invalid(reason)) if reason == "progress digest mismatch"
        ));
        sessions.advance(&first, &start, &root).unwrap();
        assert!(matches!(
            sessions.advance(&third, &start, &root),
            Err(ReplayVerificationError::HistoryRequired {
                expected_sequence: 1,
                verified_turns: 1,
            })
        ));
        let bad_chain =
            batch_with_previous(&start, 1, &turns[1..2], false, "wrong-previous-hash".into());
        assert!(matches!(
            sessions.advance(&bad_chain, &start, &root),
            Err(ReplayVerificationError::Invalid(reason)) if reason == "progress hash chain mismatch"
        ));
        sessions.advance(&second, &start, &root).unwrap();
        let third_reply = sessions.advance(&third, &start, &root).unwrap();
        let duplicate_reply = sessions.advance(&third, &start, &root).unwrap();
        assert!(third_reply.complete);
        assert_eq!(third_reply.verified_turns, 3);
        assert_eq!(duplicate_reply.verified_turns, third_reply.verified_turns);
        assert_eq!(duplicate_reply.terminal_turn, third_reply.terminal_turn);
        assert_eq!(
            duplicate_reply.participants.len(),
            third_reply.participants.len()
        );
        let slot = sessions.slot(&start.match_id).unwrap();
        let entry = slot.lock().unwrap();
        assert_eq!(entry.progress.as_ref().unwrap().turns, 3);
        assert_eq!(
            entry
                .progress
                .as_ref()
                .unwrap()
                .progress_cursor
                .as_ref()
                .unwrap()
                .sequence,
            2
        );
        drop(entry);
        drop(slot);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn current_checkpoint_recovers_previous_copy_and_resumes_its_exact_cursor() {
        let root = temporary("current-format-restart");
        let start = fixture("424263", &root);
        let saved_root = root.join("checkpoints");
        let sessions = Sessions::at(saved_root.clone(), RESIDENT_BYTES);
        let turns = (0..5)
            .map(|turn_number| Turn {
                turn_number,
                intents: Vec::new(),
            })
            .collect::<Vec<_>>();
        let first = batch(&start, 0, &turns, false);
        sessions.advance(&first, &start, &root).unwrap();
        {
            let slot = sessions.slot(&start.match_id).unwrap();
            sessions
                .save(&start.match_id, &slot.lock().unwrap())
                .unwrap();
        }
        let tail = batch_with_previous(
            &start,
            1,
            &[Turn {
                turn_number: 5,
                intents: Vec::new(),
            }],
            false,
            first.sha256,
        );
        let expected = sessions.advance(&tail, &start, &root).unwrap();
        {
            let slot = sessions.slot(&start.match_id).unwrap();
            sessions
                .save(&start.match_id, &slot.lock().unwrap())
                .unwrap();
        }
        let path = saved_root.join("424263.checkpoint");
        assert_eq!(
            bincode::deserialize::<u32>(&std::fs::read(&path).unwrap()[..4]).unwrap(),
            4
        );
        drop(sessions);
        std::fs::write(&path, b"interrupted saved copy").unwrap();
        let recovered = Sessions::at(saved_root.clone(), RESIDENT_BYTES)
            .advance(&tail, &start, &root)
            .unwrap();
        assert_eq!(
            serde_json::to_value(recovered).unwrap(),
            serde_json::to_value(expected).unwrap()
        );
        let fresh_root = root.join("retired-format");
        std::fs::create_dir_all(&fresh_root).unwrap();
        std::fs::write(
            fresh_root.join("424263.checkpoint"),
            bincode::serialize(&2u32).unwrap(),
        )
        .unwrap();
        assert!(
            Sessions::at(fresh_root, RESIDENT_BYTES)
                .advance(&tail, &start, &root)
                .is_err(),
            "retired checkpoint formats must never be interpreted as current state"
        );
        std::fs::remove_dir_all(root).unwrap();
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
    fn verified_defeat_closes_without_an_exit_request_while_others_keep_playing() {
        let root = temporary("automatic-defeat");
        let mut start = fixture("424266", &root);
        let mut third = start.metadata["players"][0].clone();
        third["id"] = serde_json::json!(3);
        start.metadata["players"]
            .as_array_mut()
            .unwrap()
            .push(third);
        let account = "00000000000000000000000000000003";
        start.player_ids.push(account.into());
        start.metadata["relay_players"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "player_id":3,"database_account_id":account,"leader":"Caesar","is_internal":false,
            }));
        let sessions = Sessions::at(root.join("checkpoints"), RESIDENT_BYTES);
        let prefix = batch(
            &start,
            0,
            &[Turn {
                turn_number: 0,
                intents: vec![
                    StampedIntent {
                        player_id: 1,
                        intent: GameplayIntent::Spawn { x: 10, y: 10 },
                    },
                    StampedIntent {
                        player_id: 2,
                        intent: GameplayIntent::Spawn { x: 25, y: 28 },
                    },
                    StampedIntent {
                        player_id: 3,
                        intent: GameplayIntent::Spawn { x: 65, y: 20 },
                    },
                ],
            }],
            false,
        );
        sessions.advance(&prefix, &start, &root).unwrap();
        {
            // Model an already-verified elimination in the shared state. The
            // next turn has no leave request or other client-supplied result.
            let slot = sessions.slot(&start.match_id).unwrap();
            let mut resident = slot.lock().unwrap();
            let engine = &mut resident.progress.as_mut().unwrap().engine;
            let player = engine
                .state
                .players
                .iter_mut()
                .find(|player| player.id == 2)
                .unwrap();
            player.alive = false;
            player.has_spawned = true;
        }
        let next = batch_with_previous(
            &start,
            1,
            &[Turn {
                turn_number: 1,
                intents: Vec::new(),
            }],
            false,
            prefix.sha256,
        );
        let reply = sessions.advance(&next, &start, &root).unwrap();
        assert!(
            reply.result.is_none(),
            "other participants must remain in play"
        );
        let outcome = reply
            .participants
            .iter()
            .find(|outcome| outcome.account_id == start.player_ids[1])
            .unwrap();
        assert_eq!(outcome.reason, "defeated");
        assert_eq!(outcome.result.personal_closure_turn, Some(1));
        assert!(!outcome.result.participants[0].won);
        let repeated = sessions.advance(&next, &start, &root).unwrap();
        assert_eq!(
            serde_json::to_value(repeated).unwrap(),
            serde_json::to_value(reply).unwrap()
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn two_hour_128_human_accounts_share_simulation_and_twenty_concurrent_payments() {
        use sow_data::{
            db::{AccountKind, PlayerDb},
            profile::{ProgressClosure, ReplayVerificationStatus},
        };
        let root = temporary("128-human-accounts");
        std::fs::create_dir_all(&root).unwrap();
        let db = PlayerDb::new(
            "redis://127.0.0.1/",
            None,
            Some(Arc::new(
                sow_data::redb_store::init_database(root.join("accounts.redb")).unwrap(),
            )),
        );
        let mut accounts = Vec::new();
        for id in 1..=128 {
            let account = db
                .get_or_create_anonymous(None, Some(&format!("Human {id}")), None)
                .await
                .unwrap();
            assert_eq!(account.kind, AccountKind::Human);
            accounts.push(account.id);
        }
        let mut start = fixture("424264", &root);
        let map = sow_core::map_file::MapFile {
            display_name: "128 Human Arena".into(),
            width: 256,
            height: 128,
            num_land_tiles: 256 * 128,
            spawns: Vec::new(),
            geo_bounds: None,
            default_roster: None,
            rosters: Vec::new(),
            terrain: vec![128; 256 * 128],
        };
        std::fs::create_dir_all(root.join("128_human_arena")).unwrap();
        std::fs::write(
            root.join("128_human_arena/map.bin"),
            sow_core::map_file::encode(&map),
        )
        .unwrap();
        let config = GameConfig {
            map_name: "128 Human Arena".into(),
            map_width: 256,
            map_height: 128,
            bot_count: 0,
            nation_count: 0,
            random_spawn: false,
            map_control_win_percentage: 0.99,
            ..Default::default()
        };
        let template: PlayerInfo =
            serde_json::from_value(start.metadata["players"][0].clone()).unwrap();
        let players = (1u16..=128)
            .map(|id| PlayerInfo {
                id,
                name: format!("Human {id}"),
                is_ai_controlled: false,
                ..template.clone()
            })
            .collect::<Vec<_>>();
        let roster = players.iter().zip(&accounts).map(|(player,account)| serde_json::json!({
            "player_id":player.id,"database_account_id":account,"leader":player.leader,"is_internal":false
        })).collect::<Vec<_>>();
        start.player_ids = accounts.clone();
        start.metadata["players"] = serde_json::to_value(&players).unwrap();
        start.metadata["relay_players"] = serde_json::to_value(roster).unwrap();
        start.metadata["config"] = serde_json::to_value(config).unwrap();
        db.register_match_start(&start.match_id, &start.player_ids, start.metadata.clone())
            .await
            .unwrap();
        let count = 72_000u64;
        let turns = (0..count)
            .map(|turn_number| Turn {
                turn_number,
                intents: if turn_number == 0 {
                    (1u16..=128)
                        .map(|id| StampedIntent {
                            player_id: id,
                            intent: GameplayIntent::Spawn {
                                x: 8 + ((id - 1) % 16) as u32 * 16,
                                y: 8 + ((id - 1) / 16) as u32 * 16,
                            },
                        })
                        .collect()
                } else {
                    Vec::new()
                },
            })
            .collect::<Vec<_>>();
        let sessions = Sessions::at(root.join("checkpoints"), RESIDENT_BYTES);
        super::super::VERIFIED_TURN_EXECUTIONS.with(|n| n.set(0));
        let started = Instant::now();
        let mut previous = String::new();
        for (sequence, chunk) in turns.chunks(50).enumerate() {
            let batch = batch_with_previous(&start, sequence as u64, chunk, false, previous);
            loop {
                let reply = sessions.advance(&batch, &start, &root).unwrap();
                assert!(
                    reply.result.is_none(),
                    "fixture must remain active for two hours"
                );
                assert!(reply.participants.is_empty());
                if reply.complete {
                    break;
                }
            }
            previous = batch.sha256;
        }
        let prefix_ms = started.elapsed().as_millis();
        assert_eq!(
            super::super::VERIFIED_TURN_EXECUTIONS.with(|n| n.get()),
            count
        );
        let close_turn = Turn {
            turn_number: count,
            intents: (1..=20)
                .map(|player_id| StampedIntent {
                    player_id,
                    intent: GameplayIntent::Resign,
                })
                .collect(),
        };
        let mut close = batch_with_previous(&start, 1440, &[close_turn], false, previous);
        close.participant_closures = accounts[..20]
            .iter()
            .map(|account| ProgressClosure {
                account_id: account.clone(),
                turn_number: count,
                reason: "voluntary".into(),
            })
            .collect();
        close.sha256 = hex::encode(Sha256::digest(close.signing_bytes().unwrap()));
        let started = Instant::now();
        let reply = sessions.advance(&close, &start, &root).unwrap();
        assert!(reply.complete);
        assert_eq!(reply.verified_turns, count + 1);
        assert_eq!(reply.participants.len(), 20);
        assert_eq!(
            super::super::VERIFIED_TURN_EXECUTIONS.with(|n| n.get()),
            count + 1,
            "twenty exits must share one additional simulated turn"
        );
        let closing_ms = started.elapsed().as_millis();
        db.apply_verified_match_participant_results(&reply.participants, None)
            .await
            .unwrap();
        let mut paid = Vec::new();
        for account in &accounts[..20] {
            let profile = db
                .account_with_events(account, &[&start.match_id])
                .await
                .unwrap()
                .profile;
            let receipt = &profile.reward_receipts[&start.match_id];
            assert_eq!(receipt.personal_closure_turn, Some(count));
            assert_eq!(
                receipt.verification_status,
                Some(ReplayVerificationStatus::Verified)
            );
            paid.push(profile);
        }
        let barrier = Arc::new(std::sync::Barrier::new(20));
        std::thread::scope(|scope| {
            for outcome in &reply.participants {
                let db = &db;
                let barrier = barrier.clone();
                scope.spawn(move || {
                    barrier.wait();
                    tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .unwrap()
                        .block_on(db.apply_verified_match_participant_results(
                            std::slice::from_ref(outcome),
                            None,
                        ))
                        .unwrap();
                });
            }
        });
        let duplicate = sessions.advance(&close, &start, &root).unwrap();
        assert_eq!(
            serde_json::to_value(&duplicate).unwrap(),
            serde_json::to_value(&reply).unwrap()
        );
        assert_eq!(
            super::super::VERIFIED_TURN_EXECUTIONS.with(|n| n.get()),
            count + 1,
            "duplicate deliveries must execute zero turns"
        );
        for (account, expected) in accounts[..20].iter().zip(paid) {
            assert_eq!(
                db.account_with_events(account, &[&start.match_id])
                    .await
                    .unwrap()
                    .profile,
                expected
            );
        }
        let output = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../dist/.sow-state/two-hour-human-exit-benchmark.json");
        std::fs::create_dir_all(output.parent().unwrap()).unwrap();
        std::fs::write(output, serde_json::to_vec_pretty(&serde_json::json!({
            "human_accounts":128,"internal_bots":0,"active_seconds":7200,"prefix_turns":count,
            "simultaneous_exits":20,"closing_turns_executed":1,"duplicate_turns_executed":0,
            "prefix_ms":prefix_ms,"closing_ms":closing_ms,"concurrent_duplicate_payments_unchanged":true,
        })).unwrap()).unwrap();
        drop(sessions);
        drop(db);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn four_hour_active_matches_interleave_spill_restore_and_only_check_the_closing_tail() {
        let root = temporary("four-hours");
        let first = fixture("424250", &root);
        let mut second = first.clone();
        second.match_id = "424251".into();
        let sessions = Sessions::at(root.join("checkpoints"), RESIDENT_BYTES);
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
        let mut previous_hashes = HashMap::from([
            (first.match_id.clone(), String::new()),
            (second.match_id.clone(), String::new()),
        ]);
        for (sequence, chunk) in turns.chunks(50).enumerate() {
            for start in [&first, &second] {
                let batch = batch_with_previous(
                    start,
                    sequence as u64,
                    chunk,
                    false,
                    previous_hashes[&start.match_id].clone(),
                );
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
                previous_hashes.insert(start.match_id.clone(), batch.sha256.clone());
            }
            if sequence == 100 || sequence == 1500 {
                sessions.budget.store(1, Ordering::Relaxed);
                sessions.trim().unwrap();
                assert!(
                    sessions.resident.lock().unwrap().is_empty(),
                    "both interleaved matches must spill to disk"
                );
                sessions.budget.store(RESIDENT_BYTES, Ordering::Relaxed);
            }
        }
        let simulated_ms = started.elapsed().as_millis();
        let slot = sessions.slot(&first.match_id).unwrap();
        let first_entry = slot.lock().unwrap();
        assert_eq!(
            first_entry.progress.as_ref().unwrap().engine.state.tick,
            count
        );
        let checkpoint_bytes = sessions.save(&first.match_id, &first_entry).unwrap();
        drop(first_entry);
        drop(slot);
        let restarted = Sessions::at(sessions.root.clone(), RESIDENT_BYTES);
        let close = [Turn {
            turn_number: count,
            intents: vec![StampedIntent {
                player_id: 1,
                intent: GameplayIntent::Resign,
            }],
        }];
        let before = Instant::now();
        let checked = restarted
            .advance(
                &batch_with_previous(
                    &first,
                    2880,
                    &close,
                    true,
                    previous_hashes[&first.match_id].clone(),
                ),
                &first,
                &root,
            )
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
        let cold_sessions = Sessions::at(root.join("cold-reference"), RESIDENT_BYTES);
        let mut cold_previous = String::new();
        let mut cold_result = None;
        for (sequence, chunk) in full.chunks(50).enumerate() {
            let final_chunk = chunk.last().unwrap().turn_number == count;
            let cold_batch =
                batch_with_previous(&first, sequence as u64, chunk, final_chunk, cold_previous);
            loop {
                let reply = cold_sessions.advance(&cold_batch, &first, &root).unwrap();
                if reply.complete {
                    cold_result = reply.result;
                    break;
                }
            }
            cold_previous = cold_batch.sha256;
        }
        let cold = cold_result.unwrap();
        assert_eq!(
            checked.result.unwrap(),
            cold,
            "restarted incremental result differs from full verification"
        );
        let output = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../dist/.sow-state/progressive-verification-benchmark.json");
        std::fs::create_dir_all(output.parent().unwrap()).unwrap();
        std::fs::write(output,serde_json::to_vec_pretty(&serde_json::json!({"active_match_seconds":14400,"concurrent_matches":2,"turns_per_match":count,"participants_per_match":2,"closing_turns":1,"simulation_ms":simulated_ms,"closing_restore_ms":closing_ms,"checkpoint_bytes":checkpoint_bytes})).unwrap()).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn team_finish_pays_both_human_winners_without_waiting_for_their_exit() {
        use sow_core::protocol::Team;
        use sow_data::db::PlayerDb;
        let root = temporary("team-final-payments");
        std::fs::create_dir_all(&root).unwrap();
        let db = PlayerDb::new(
            "redis://127.0.0.1/",
            None,
            Some(Arc::new(
                sow_data::redb_store::init_database(root.join("accounts.redb")).unwrap(),
            )),
        );
        let mut accounts = Vec::new();
        for id in 1..=3 {
            accounts.push(
                db.get_or_create_anonymous(None, Some(&format!("Team human {id}")), None)
                    .await
                    .unwrap(),
            );
        }
        let mut start = fixture("424265", &root);
        start.metadata["config"]["game_mode"] = serde_json::json!("Teams");
        let template: PlayerInfo =
            serde_json::from_value(start.metadata["players"][0].clone()).unwrap();
        let players = (1u16..=3)
            .map(|id| PlayerInfo {
                id,
                team: Some(if id == 2 { Team::Blue } else { Team::Red }),
                ..template.clone()
            })
            .collect::<Vec<_>>();
        start.player_ids = accounts.iter().map(|account| account.id.clone()).collect();
        start.metadata["players"] = serde_json::to_value(&players).unwrap();
        start.metadata["relay_players"] = serde_json::json!(players.iter().zip(&accounts).map(|(player,account)|serde_json::json!({
            "player_id":player.id,"database_account_id":account.id,"leader":player.leader,"team":player.team,"is_internal":false,
        })).collect::<Vec<_>>());
        db.register_match_start(&start.match_id, &start.player_ids, start.metadata.clone())
            .await
            .unwrap();
        let sessions = Sessions::at(root.join("checkpoints"), RESIDENT_BYTES);
        let turns = (0..300)
            .map(|turn_number| Turn {
                turn_number,
                intents: if turn_number == 0 {
                    vec![
                        StampedIntent {
                            player_id: 1,
                            intent: GameplayIntent::Spawn { x: 12, y: 8 },
                        },
                        StampedIntent {
                            player_id: 2,
                            intent: GameplayIntent::Spawn { x: 65, y: 20 },
                        },
                        StampedIntent {
                            player_id: 3,
                            intent: GameplayIntent::Spawn { x: 25, y: 30 },
                        },
                    ]
                } else {
                    Vec::new()
                },
            })
            .collect::<Vec<_>>();
        let prefix = batch(&start, 0, &turns, false);
        while !sessions.advance(&prefix, &start, &root).unwrap().complete {}
        let tail = batch_with_previous(
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
            prefix.sha256,
        );
        let reply = sessions.advance(&tail, &start, &root).unwrap();
        assert_eq!(reply.terminal_turn, Some(300));
        let result = reply
            .result
            .as_ref()
            .expect("team victory must be ready before winners leave");
        assert_eq!(result.winning_team.as_deref(), Some("Red"));
        assert_eq!(
            result
                .participants
                .iter()
                .filter(|participant| participant.won)
                .map(|participant| participant.player_id)
                .collect::<Vec<_>>(),
            vec![1, 3]
        );
        db.apply_verified_match_participant_results(&reply.participants, None)
            .await
            .unwrap();
        db.apply_verified_match_result(result, None).await.unwrap();
        let mut paid = Vec::new();
        for (i, account) in accounts.iter().enumerate() {
            let current = db
                .account_with_events(&account.id, &[&start.match_id])
                .await
                .unwrap();
            let reward = sow_data::rewards::calculate(sow_data::rewards::RewardInput {
                won: i != 1,
                ..Default::default()
            });
            assert_eq!(current.profile.xp, account.profile.xp + reward.xp);
            paid.push(current.profile);
        }
        db.apply_verified_match_result(result, None).await.unwrap();
        for (account, expected) in accounts.iter().zip(paid) {
            assert_eq!(
                db.account_with_events(&account.id, &[&start.match_id])
                    .await
                    .unwrap()
                    .profile,
                expected
            );
        }
        drop(sessions);
        drop(db);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn natural_finish_ignores_registered_actions_after_game_over() {
        let root = temporary("outcomes");
        let start = fixture("424252", &root);
        let sessions = Sessions::at(root.join("checkpoints"), RESIDENT_BYTES);
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
        let last = batch_with_previous(
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
            first.sha256.clone(),
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
        assert!(
            finished
                .participants
                .iter()
                .all(|outcome| outcome.result.participants.len() == 1),
            "a participant result must contain only that participant's statistics"
        );
        assert!(
            sessions.advance(&first, &start, &root).is_err(),
            "an old block cannot claim a later closure or victory"
        );

        let trailing = batch_with_previous(
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
            last.sha256.clone(),
        );
        let after_game_over = sessions.advance(&trailing, &start, &root).unwrap();
        assert!(after_game_over.complete);
        assert_eq!(after_game_over.verified_turns, 302);
        assert_eq!(after_game_over.terminal_turn, Some(300));
        assert!(
            after_game_over.result.is_none(),
            "the final result belongs to its original block"
        );
        assert!(
            after_game_over.participants.is_empty(),
            "closed participants must not be sent again in later blocks"
        );

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

        let invalid_sender_sessions =
            Sessions::at(root.join("invalid-sender-checkpoints"), RESIDENT_BYTES);
        while !invalid_sender_sessions
            .advance(&first, &start, &root)
            .unwrap()
            .complete
        {}
        invalid_sender_sessions
            .advance(&last, &start, &root)
            .unwrap();
        let unregistered_tail = batch_with_previous(
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
            last.sha256.clone(),
        );
        assert!(
            invalid_sender_sessions
                .advance(&unregistered_tail, &start, &root)
                .is_err()
        );
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
        Err(error) => {
            log::warn!("[rewards] invalid binary progress request: {error}");
            return StatusCode::BAD_REQUEST.into_response();
        }
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
            .advance(&batch, &start, &state.maps_root)
    })
    .await
    {
        Ok(Ok(reply)) => (StatusCode::OK, Json(reply)).into_response(),
        Ok(Err(ReplayVerificationError::HistoryRequired {
            expected_sequence,
            verified_turns,
        })) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "error": "verified history required",
                "expected_sequence": expected_sequence,
                "verified_turns": verified_turns,
            })),
        )
            .into_response(),
        Ok(Err(error)) => {
            let invalid = matches!(&error, ReplayVerificationError::Invalid(_));
            log::warn!("[rewards] progress verification pending: {error}");
            (
                if invalid {
                    StatusCode::UNPROCESSABLE_ENTITY
                } else {
                    StatusCode::SERVICE_UNAVAILABLE
                },
                Json(serde_json::json!({"error":format!("{error}")})),
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
