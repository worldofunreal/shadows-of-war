//! One durable owner for return commands, personal results and whole replays.
use super::*;
use sha2::{Digest, Sha256};
use sow_data::profile::{
    MatchExitState, MatchWork, MatchWorkRecord, ReplayVerificationStatus, RewardSettlementStatus,
};
use std::path::PathBuf;

type WorkResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Serialize,Deserialize)]
struct ProgressIndex {
    match_id:String,
    chunks:Vec<ProgressIndexChunk>,
}

#[derive(Serialize,Deserialize)]
struct ProgressIndexChunk {
    sequence:u64,
    first_turn:u64,
    turn_count:u64,
    previous_sha256:String,
    sha256:String,
    finished:bool,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn participant_path(match_id: &str, account_id: &str) -> PathBuf {
    PathBuf::from(std::env::var("SOW_REPLAY_DIR").unwrap_or_else(|_| "replays".to_string()))
        .join(format!("{match_id}-{account_id}.participant.replay"))
}

fn progress_path(match_id: &str, sequence: u64) -> PathBuf {
    progress_path_with_extension(match_id, sequence, "bin")
}

fn progress_path_with_extension(match_id: &str, sequence: u64, extension: &str) -> PathBuf {
    participant_path(match_id, "").with_file_name(format!("{match_id}.progress.{sequence:010}.{extension}"))
}

fn existing_progress_path(match_id: &str, sequence: u64) -> PathBuf {
    let binary = progress_path(match_id, sequence);
    if binary.exists() { return binary; }
    let json = progress_path_with_extension(match_id, sequence, "json");
    if json.exists() { json } else { binary }
}

fn decode_progress(bytes: &[u8]) -> WorkResult<(sow_data::profile::MatchProgress, bool)> {
    match bincode::deserialize(bytes) {
        Ok(batch) => Ok((batch, true)),
        Err(binary_error) => match serde_json::from_slice(bytes) {
            Ok(batch) => Ok((batch, false)),
            Err(json_error) => Err(format!("invalid progress record (binary: {binary_error}; JSON: {json_error})").into()),
        },
    }
}

fn progress_index_path(match_id:&str) -> PathBuf {
    progress_path(match_id,0).with_file_name(format!("{match_id}.progress.index"))
}

async fn read_progress_index(match_id:&str) -> WorkResult<Option<ProgressIndex>> {
    match tokio::fs::read(progress_index_path(match_id)).await {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn participant_result_path(match_id:&str, account_id:&str, turn:u64) -> PathBuf {
    participant_path(match_id,account_id).with_extension(format!("closure.{turn}.paid"))
}

fn match_result_path(match_id:&str) -> PathBuf {
    progress_path(match_id,0).with_file_name(format!("{match_id}.result.paid"))
}

async fn durable_marker(path:&std::path::Path, value:&str) -> WorkResult<()> {
    if tokio::fs::read_to_string(path).await.is_ok_and(|saved| saved == value) { return Ok(()); }
    let temporary = path.with_extension(format!("{}.tmp",rand::random::<u64>()));
    let mut file = tokio::fs::File::create(&temporary).await?;
    file.write_all(value.as_bytes()).await?;
    file.sync_all().await?;
    drop(file);
    tokio::fs::rename(&temporary,path).await?;
    std::fs::File::open(path.parent().ok_or("result marker directory missing")?)?.sync_all()?;
    Ok(())
}

fn marker_matches(path:&std::path::Path, value:&str) -> WorkResult<bool> {
    match std::fs::read_to_string(path) {
        Ok(saved) => Ok(saved == value),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

async fn archived_chunk_matches(match_id:&str,sequence:u64,sha256:&str) -> WorkResult<bool> {
    let Some(index) = read_progress_index(match_id).await? else { return Ok(false); };
    if index.match_id != match_id || !index.chunks.iter().any(|chunk| chunk.sequence == sequence && chunk.sha256 == sha256) {
        return Ok(false);
    }
    let Some(last) = index.chunks.last() else { return Ok(false); };
    marker_matches(&progress_path(match_id,last.sequence).with_extension("archived"),&last.sha256)
}

async fn progress_index(match_id:&str,last_sequence:u64) -> WorkResult<ProgressIndex> {
    let mut chunks = Vec::with_capacity(last_sequence as usize+1);
    let mut first_turn = 0u64;
    let mut previous = String::new();
    for sequence in 0..=last_sequence {
        let path = existing_progress_path(match_id,sequence);
        let (batch, _) = decode_progress(&tokio::fs::read(path).await?)?;
        if batch.match_id != match_id || batch.sequence != sequence || batch.first_turn != first_turn
            || batch.previous_sha256 != previous || batch.replay_data.len() < 8
            || hex::encode(Sha256::digest(batch.signing_bytes()?)) != batch.sha256
            || (sequence < last_sequence && batch.finished) {
            return Err("cannot archive a broken journal sequence".into());
        }
        let count = u64::from_le_bytes(batch.replay_data[..8].try_into().unwrap());
        chunks.push(ProgressIndexChunk { sequence,first_turn,turn_count:count,
            previous_sha256:previous,sha256:batch.sha256.clone(),finished:batch.finished });
        first_turn = first_turn.saturating_add(count);
        previous = batch.sha256;
    }
    if !chunks.last().is_some_and(|chunk|chunk.finished) { return Err("cannot archive an unfinished journal".into()); }
    Ok(ProgressIndex { match_id:match_id.to_string(),chunks })
}

async fn prune_progress_chunks(match_id:&str,last_sequence:u64) -> WorkResult<()> {
    let final_marker = progress_path(match_id,last_sequence).with_extension("archived");
    let directory = progress_path(match_id,0).parent().ok_or("progress directory missing")?.to_path_buf();
    let prefix = format!("{match_id}.progress.");
    let mut entries = tokio::fs::read_dir(&directory).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|value|value.to_str()) else { continue; };
        if !name.starts_with(&prefix) || path == final_marker { continue; }
        let suffix = &name[prefix.len()..];
        if suffix.ends_with(".bin") || suffix.ends_with(".json") || suffix.ends_with(".done") || suffix.ends_with(".tmp") || suffix.ends_with(".archived") {
            tokio::fs::remove_file(path).await?;
        }
    }
    std::fs::File::open(directory)?.sync_all()?;
    Ok(())
}

pub(super) async fn handle_progress(State(state): State<Arc<AppState>>, headers: HeaderMap,
    body: Bytes) -> axum::response::Response {
    if !verify_internal_auth(&headers, &state.relay_settlement_secret) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let (batch, binary) = match decode_progress(&body) {
        Ok(decoded) => decoded,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let match_id = batch.match_id.clone();
    let result = store_progress(&state, batch, binary).await;
    match result {
        Ok::<_,Box<dyn std::error::Error+Send+Sync>>(ack) => (StatusCode::ACCEPTED,Json(ack)).into_response(),
        Err(e) => {
            warn!("[rewards] journal intake pending match={match_id}: {e}");
            (StatusCode::SERVICE_UNAVAILABLE,Json(ErrorResponse { error:e.to_string() })).into_response()
        }
    }
}

pub(super) async fn handle_relay_progress(State(state): State<Arc<AppState>>, headers: HeaderMap,
    body: Bytes) -> axum::response::Response {
    if !verify_internal_auth(&headers, &state.relay_settlement_secret) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let incoming: sow_data::profile::RelayMatchProgress = match bincode::deserialize(&body) {
        Ok(batch) => batch,
        Err(binary_error) => match serde_json::from_slice(&body) {
            Ok(batch) => batch,
            Err(_) => {
                warn!("[rewards] invalid relay progress request: {binary_error}");
                return StatusCode::BAD_REQUEST.into_response();
            }
        },
    };
    if incoming.match_id.parse::<u64>().is_err() {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let result = async {
        let _guard = state.match_artifact_lock.lock().await;
        if incoming.recording_failed {
            return reject_relay_recording(&state, &incoming).await;
        }
        if incoming.replay_data.len() < 8
            || incoming.replay_data.len() > sow_data::MAX_MATCH_CHUNK_BYTES
        {
            return Err("invalid relay turn batch".into());
        }
        let count = u64::from_le_bytes(incoming.replay_data[..8].try_into().unwrap());
        let previous_sha256 = previous_progress_hash(&incoming.match_id, incoming.sequence).await?;
        let mut batch = sow_data::profile::MatchProgress {
            match_id: incoming.match_id.clone(),
            sequence: incoming.sequence,
            first_turn: incoming.first_turn,
            previous_sha256,
            sha256: String::new(),
            replay_data: incoming.replay_data.clone(),
            participant_closures: incoming.participant_closures.clone(),
            finished: incoming.finished,
        };
        batch.sha256 = hex::encode(Sha256::digest(batch.signing_bytes()?));
        store_progress_locked(&state, batch, true, count).await
    }.await;
    match result {
        Ok(ack) => (StatusCode::ACCEPTED, Json(ack)).into_response(),
        Err(error) => {
            warn!("[rewards] relay progress intake pending match={}: {error}", incoming.match_id);
            (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error:error.to_string() })).into_response()
        }
    }
}

async fn store_progress(
    state: &AppState,
    batch: sow_data::profile::MatchProgress,
    binary: bool,
) -> WorkResult<serde_json::Value> {
    let _guard = state.match_artifact_lock.lock().await;
    let count = u64::from_le_bytes(batch.replay_data[..8].try_into().map_err(|_| "invalid turn count")?);
    store_progress_locked(state, batch, binary, count).await
}

async fn store_progress_locked(
    state: &AppState,
    batch: sow_data::profile::MatchProgress,
    binary: bool,
    count: u64,
) -> WorkResult<serde_json::Value> {
    if batch.match_id.parse::<u64>().is_err() || batch.replay_data.len() > sow_data::MAX_MATCH_CHUNK_BYTES
        || batch.replay_data.len() < 8 || hex::encode(Sha256::digest(batch.signing_bytes()?)) != batch.sha256 {
        return Err("invalid journal delivery".into());
    }
    let start = state.db.match_start_record(&batch.match_id)?.ok_or("trusted match registration unavailable")?;
    let tick_rate = start.metadata.pointer("/config/tick_rate_ms").and_then(serde_json::Value::as_f64)
        .ok_or("trusted clock missing")? as f32;
    if !tick_rate.is_finite() || tick_rate <= 0.0 || count > sow_data::max_match_turns(tick_rate)
        || batch.first_turn.saturating_add(count) > sow_data::max_match_turns(tick_rate) {
        return Err("invalid journal duration".into());
    }
    let canonical = progress_path(&batch.match_id, batch.sequence);
    let legacy = progress_path_with_extension(&batch.match_id, batch.sequence, "json");
    let path = if canonical.exists() { canonical } else if legacy.exists() { legacy }
        else if binary { canonical } else { legacy };
    let existing = tokio::fs::read(&path).await;
    match existing {
        Ok(bytes) => {
            let (saved, _) = decode_progress(&bytes)?;
            if saved != batch { return Err("journal delivery conflicts with saved state".into()); }
            if !marker_matches(&path.with_extension("done"),&saved.sha256)? {
                let job = state.db.queue_match_work(progress_work(&batch))?;
                wake(state,job.work.key());
            } else if batch.finished && !marker_matches(&path.with_extension("archived"),&batch.sha256)? {
                let job = state.db.queue_match_work(archive_work(&batch))?;
                wake(state,job.work.key());
            }
            return Ok(serde_json::json!({"match_id":batch.match_id,"sequence":batch.sequence,"sha256":batch.sha256}));
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if batch.finished && marker_matches(&path.with_extension("archived"),&batch.sha256)? {
                return Ok(serde_json::json!({"match_id":batch.match_id,"sequence":batch.sequence,"sha256":batch.sha256}));
            }
            if let Some(index) = read_progress_index(&batch.match_id).await? {
                let saved = index.match_id == batch.match_id && index.chunks.iter().any(|chunk|
                    chunk.sequence == batch.sequence && chunk.first_turn == batch.first_turn
                        && chunk.previous_sha256 == batch.previous_sha256 && chunk.sha256 == batch.sha256
                        && chunk.finished == batch.finished);
                let archived = if let Some(chunk) = index.chunks.last() {
                    marker_matches(&progress_path(&batch.match_id,chunk.sequence).with_extension("archived"),&chunk.sha256)?
                } else { false };
                if saved && archived {
                    return Ok(serde_json::json!({"match_id":batch.match_id,"sequence":batch.sequence,"sha256":batch.sha256}));
                }
                return Err("journal delivery conflicts with its archived record".into());
            }
        }
        Err(e) => return Err(e.into()),
    }
    if batch.sequence == 0 {
        if batch.first_turn != 0 || !batch.previous_sha256.is_empty() { return Err("journal must start at turn zero".into()); }
    } else {
        let bytes = tokio::fs::read(existing_progress_path(&batch.match_id,batch.sequence-1)).await?;
        let (previous, _) = decode_progress(&bytes)?;
        let previous_count = u64::from_le_bytes(previous.replay_data[..8].try_into().unwrap());
        if previous.sha256 != batch.previous_sha256 || previous.first_turn + previous_count != batch.first_turn || previous.finished {
            return Err("journal delivery conflicts with saved sequence".into());
        }
    }
    for closure in &batch.participant_closures {
        state.db.registered_human_participant(&batch.match_id, &closure.account_id)?;
        if closure.turn_number >= batch.first_turn + count || !matches!(closure.reason.as_str(), "voluntary" | "timeout" | "match_ended" | "defeated") {
            return Err("invalid journal closure".into());
        }
    }
    let parent = path.parent().ok_or("journal directory missing")?;
    tokio::fs::create_dir_all(parent).await?;
    let temporary = path.with_extension(format!("{}.tmp", path.extension().and_then(|value|value.to_str()).unwrap_or("bin")));
    let mut file = tokio::fs::File::create(&temporary).await?;
    let encoded = if binary { bincode::serialize(&batch)? } else { serde_json::to_vec(&batch)? };
    file.write_all(&encoded).await?;
    file.sync_all().await?;
    drop(file);
    tokio::fs::rename(temporary,&path).await?;
    std::fs::File::open(parent)?.sync_all()?;
    let job = state.db.queue_match_work(progress_work(&batch))?;
    wake(state, job.work.key());
    Ok(serde_json::json!({"match_id":batch.match_id,"sequence":batch.sequence,"sha256":batch.sha256}))
}

async fn previous_progress_hash(match_id: &str, sequence: u64) -> WorkResult<String> {
    if sequence == 0 { return Ok(String::new()); }
    if let Some(index) = read_progress_index(match_id).await? {
        if index.match_id != match_id { return Err("progress index belongs to another match".into()); }
        if let Some(previous) = index.chunks.get(sequence.saturating_sub(1) as usize) {
            return Ok(previous.sha256.clone());
        }
    }
    let bytes = tokio::fs::read(existing_progress_path(match_id, sequence - 1)).await?;
    let (previous, _) = decode_progress(&bytes)?;
    Ok(previous.sha256)
}

async fn reject_relay_recording(
    state: &AppState,
    incoming: &sow_data::profile::RelayMatchProgress,
) -> WorkResult<serde_json::Value> {
    let start = state.db.match_start_record(&incoming.match_id)?.ok_or("trusted match registration unavailable")?;
    for closure in &incoming.participant_closures {
        state.db.registered_human_participant(&incoming.match_id, &closure.account_id)?;
        if closure.turn_number > sow_data::max_match_turns(
            start.metadata.pointer("/config/tick_rate_ms").and_then(serde_json::Value::as_f64)
                .ok_or("trusted clock missing")? as f32,
        ) || !matches!(closure.reason.as_str(), "voluntary" | "timeout" | "match_ended" | "defeated") {
            return Err("invalid failed-recording closure".into());
        }
    }
    if start.replay_status == ReplayVerificationStatus::Pending {
        if incoming.finished {
            state.db.finalize_match_with_lobby(&incoming.match_id, None).await?;
        }
        super::reject_queued_replay(state, &incoming.match_id).await?;
    }
    settle_rejected_progress_closures(state, &sow_data::profile::MatchProgress {
        match_id:incoming.match_id.clone(), sequence:incoming.sequence, first_turn:incoming.first_turn,
        previous_sha256:String::new(), sha256:String::new(), replay_data:Vec::new(),
        participant_closures:incoming.participant_closures.clone(), finished:incoming.finished,
    }, &[]).await?;
    durable_marker(&match_result_path(&incoming.match_id), &incoming.match_id).await?;
    let _ = state.match_results.send(incoming.match_id.clone());
    Ok(serde_json::json!({"match_id":incoming.match_id,"sequence":incoming.sequence,"recording_failed":true}))
}

fn progress_work(batch: &sow_data::profile::MatchProgress) -> MatchWork {
    MatchWork::Progress { match_id:batch.match_id.clone(), sequence:batch.sequence, sha256:batch.sha256.clone(),
        closing:batch.finished || !batch.participant_closures.is_empty() }
}

fn archive_work(batch:&sow_data::profile::MatchProgress) -> MatchWork {
    MatchWork::Archive { match_id:batch.match_id.clone(),sequence:batch.sequence,sha256:batch.sha256.clone() }
}

fn permanent_progress_rejection(status: reqwest::StatusCode) -> bool {
    matches!(status, reqwest::StatusCode::BAD_REQUEST | reqwest::StatusCode::UNPROCESSABLE_ENTITY)
}

fn invalid_verified_result(error: &dyn std::error::Error) -> bool {
    error.to_string().starts_with("invalid verified result:")
}

async fn complete_progress_chunk(
    state: &AppState,
    batch: &sow_data::profile::MatchProgress,
) -> WorkResult<bool> {
    durable_marker(&progress_path(&batch.match_id,batch.sequence).with_extension("done"),&batch.sha256).await?;
    if batch.finished {
        let job = state.db.queue_match_work(archive_work(batch))?;
        wake(state,job.work.key());
    }
    Ok(true)
}

async fn settle_rejected_progress_closures(
    state: &AppState,
    batch: &sow_data::profile::MatchProgress,
    additional: &[(String,u64)],
) -> WorkResult<()> {
    let mut closures = batch.participant_closures.iter()
        .map(|closure|(closure.account_id.clone(),closure.turn_number))
        .chain(additional.iter().cloned())
        .collect::<Vec<_>>();
    closures.sort_unstable();
    closures.dedup();
    for (account_id,turn) in closures {
        super::reject_participant_settlement(&state.db,&batch.match_id,&account_id,turn)
            .await.map_err(|error| std::io::Error::new(std::io::ErrorKind::Other,error))?;
        durable_marker(&participant_result_path(&batch.match_id,&account_id,turn),&format!("{}:{account_id}:{turn}",batch.match_id)).await?;
    }
    Ok(())
}

async fn reject_progress(
    state: &AppState,
    batch: &sow_data::profile::MatchProgress,
    additional_closures: &[(String,u64)],
    reason: &str,
) -> WorkResult<bool> {
    super::reject_queued_replay(state,&batch.match_id).await?;
    settle_rejected_progress_closures(state,batch,additional_closures).await?;
    durable_marker(&match_result_path(&batch.match_id),&batch.match_id).await?;
    warn!("[rewards] progress permanently rejected match={} sequence={} reason={reason}",batch.match_id,batch.sequence);
    complete_progress_chunk(state,batch).await
}

async fn verify_progress(state: &AppState, work: &MatchWork) -> WorkResult<bool> {
    let MatchWork::Progress { match_id, sequence, sha256, .. } = work else { return Err("invalid journal work".into()); };
    if archived_chunk_matches(match_id,*sequence,sha256).await? { return Ok(true); }
    let bytes = tokio::fs::read(existing_progress_path(match_id,*sequence)).await?;
    let (batch, binary) = decode_progress(&bytes)?;
    if &batch.sha256 != sha256 || hex::encode(Sha256::digest(batch.signing_bytes()?)) != *sha256 {
        return Err("saved journal checksum mismatch".into());
    }
    let start = state.db.match_start_record(match_id)?.ok_or("trusted match registration unavailable")?;
    match start.replay_status {
        ReplayVerificationStatus::Rejected => {
            settle_rejected_progress_closures(state,&batch,&[]).await?;
            durable_marker(&match_result_path(match_id),match_id).await?;
            return complete_progress_chunk(state,&batch).await;
        }
        ReplayVerificationStatus::Verified => {
            durable_marker(&match_result_path(match_id),match_id).await?;
            return complete_progress_chunk(state,&batch).await;
        }
        ReplayVerificationStatus::Pending => {}
    }
    let base = std::env::var("SOW_SERVER_HTTP_URL").unwrap_or_else(|_| "http://127.0.0.1:25566".into());
    let response = state.replay_verification_client.post(format!("{}/internal/match/progress",base.trim_end_matches('/')))
        .bearer_auth(&state.secret_token).header("Content-Type",if binary { "application/octet-stream" } else { "application/json" }).body(bytes).send().await?;
    if response.status() == reqwest::StatusCode::CONFLICT {
        // A restart can restore an earlier checkpoint. Requeue durable chunks;
        // checked turn hashes skip the prefix still held by the verifier.
        for prior in 0..*sequence {
            let bytes = tokio::fs::read(existing_progress_path(match_id,prior)).await?;
            let (saved, _) = decode_progress(&bytes)?;
            state.db.queue_match_work(progress_work(&saved))?;
        }
        return Err("verifier is restoring saved journal history".into());
    }
    if permanent_progress_rejection(response.status()) {
        let status = response.status();
        let reason = response.text().await.unwrap_or_else(|error|format!("response unavailable: {error}"));
        return reject_progress(state,&batch,&[],&format!("verifier HTTP {status}: {reason}")).await;
    }
    if !response.status().is_success() { return Err(format!("journal verifier HTTP {}: {}", response.status(),response.text().await?).into()); }
    let checked = response.json::<sow_data::profile::VerifiedProgress>().await?;
    if checked.match_id != *match_id || checked.sequence != *sequence || checked.sha256 != *sha256 {
        return reject_progress(state,&batch,&[],"verifier returned another journal delivery").await;
    }
    let mut closures = Vec::new();
    for outcome in &checked.participants {
        if outcome.result.match_id != *match_id { return reject_progress(state,&batch,&[],"verifier returned another match").await; }
        let Some(turn) = outcome.result.personal_closure_turn else { return reject_progress(state,&batch,&[],"verified closure turn missing").await; };
        state.db.note_match_participant_closure(match_id,&outcome.account_id,turn).await?;
        if let Err(error) = state.db.apply_verified_match_participant_result(&outcome.result,&outcome.account_id,turn).await {
            if invalid_verified_result(error.as_ref()) {
                let closures = checked.participants.iter().filter_map(|item|
                    item.result.personal_closure_turn.map(|turn|(item.account_id.clone(),turn)))
                    .collect::<Vec<_>>();
                return reject_progress(state,&batch,&closures,&error.to_string()).await;
            }
            return Err(error);
        }
        durable_marker(&participant_result_path(match_id,&outcome.account_id,turn),&format!("{match_id}:{}:{turn}",outcome.account_id)).await?;
        closures.push(sow_data::profile::ProgressClosure { account_id:outcome.account_id.clone(),turn_number:turn,reason:outcome.reason.clone() });
    }
    if let Some(result) = &checked.result {
        state.db.finalize_match_with_lobby(match_id,None).await?;
        if let Err(error) = state.db.apply_verified_match_result(result).await {
            if invalid_verified_result(error.as_ref()) {
                let participant_closures = checked.participants.iter().filter_map(|item|
                    item.result.personal_closure_turn.map(|turn|(item.account_id.clone(),turn)))
                    .collect::<Vec<_>>();
                return reject_progress(state,&batch,&participant_closures,&error.to_string()).await;
            }
            return Err(error);
        }
        state.db.finish_replay_verification(match_id,ReplayVerificationStatus::Verified)?;
        durable_marker(&match_result_path(match_id),match_id).await?;
    }
    let _ = state.match_results.send(match_id.clone());
    if checked.terminal_turn.is_some() || !closures.is_empty() {
        let response = state.replay_verification_client.post(format!("{}/internal/match/result",base.trim_end_matches('/')))
            .bearer_auth(&state.secret_token).json(&sow_data::profile::MatchResultControl {
                match_id:match_id.clone(),terminal_turn:checked.terminal_turn,participant_closures:closures }).send().await?;
        if !response.status().is_success() { return Err(format!("verified result delivery HTTP {}",response.status()).into()); }
    }
    if checked.complete {
        return complete_progress_chunk(state,&batch).await;
    }
    Ok(false)
}

async fn archive_progress(state: &AppState, last: &sow_data::profile::MatchProgress) -> WorkResult<()> {
    use tokio::io::AsyncSeekExt;
    let path = participant_path(&last.match_id, "").with_file_name(format!("{}.replay",last.match_id));
    let temporary = path.with_extension("replay.tmp");
    let mut file = tokio::fs::File::create(&temporary).await?;
    file.write_all(&0u64.to_le_bytes()).await?;
    let mut turns = 0u64;
    let mut archive_bytes = 8u64;
    for sequence in 0..=last.sequence {
        let bytes = tokio::fs::read(existing_progress_path(&last.match_id,sequence)).await?;
        let (batch, _) = decode_progress(&bytes)?;
        if batch.replay_data.len() < 8 { return Err("progress archive chunk is incomplete".into()); }
        archive_bytes = archive_bytes.saturating_add(batch.replay_data.len().saturating_sub(8) as u64);
        if archive_bytes > sow_data::MAX_MATCH_ARCHIVE_BYTES as u64 { return Err("match archive exceeds its registered size limit".into()); }
        turns += u64::from_le_bytes(batch.replay_data[..8].try_into().unwrap());
        file.write_all(&batch.replay_data[8..]).await?;
    }
    file.seek(std::io::SeekFrom::Start(0)).await?;
    file.write_all(&turns.to_le_bytes()).await?;
    file.sync_all().await?;
    drop(file);
    tokio::fs::rename(temporary,&path).await?;
    let start = state.db.match_start_record(&last.match_id)?.ok_or("trusted match registration missing")?;
    let meta = path.with_extension("json");
    let temporary = meta.with_extension("json.tmp");
    let mut file = tokio::fs::File::create(&temporary).await?;
    file.write_all(&serde_json::to_vec(&start.metadata)?).await?;
    file.sync_all().await?;
    drop(file);
    tokio::fs::rename(temporary,meta).await?;
    std::fs::File::open(path.parent().unwrap())?.sync_all()?;
    Ok(())
}

async fn archive_progress_work(state:&AppState, work:&MatchWork) -> WorkResult<bool> {
    let MatchWork::Archive { match_id,sequence,sha256 } = work else { return Err("invalid archive work".into()); };
    let artifact = existing_progress_path(match_id,*sequence);
    let archived = artifact.with_extension("archived");
    if marker_matches(&archived,sha256)? {
        let index = read_progress_index(match_id).await?.ok_or("archived journal index is missing")?;
        if index.match_id != *match_id || !index.chunks.last().is_some_and(|chunk|chunk.sequence == *sequence && chunk.sha256 == *sha256) {
            return Err("archived journal index conflicts with its final chunk".into());
        }
        prune_progress_chunks(match_id,*sequence).await?;
        return Ok(true);
    }
    let (batch, _) = decode_progress(&tokio::fs::read(&artifact).await?)?;
    if !batch.finished || batch.sha256 != *sha256 || hex::encode(Sha256::digest(batch.signing_bytes()?)) != *sha256 {
        return Err("final archive does not match its durable work".into());
    }
    let index = progress_index(match_id,*sequence).await?;
    archive_progress(state,&batch).await?;
    durable_marker(&progress_index_path(match_id),&serde_json::to_string(&index)?).await?;
    durable_marker(&archived,sha256).await?;
    prune_progress_chunks(match_id,*sequence).await?;
    Ok(true)
}

fn terminal(receipt: Option<&sow_data::profile::RewardReceipt>) -> Option<MatchExitState> {
    let receipt = receipt?;
    if !matches!(
        receipt.status,
        RewardSettlementStatus::Applied | RewardSettlementStatus::Presented
    ) {
        return None;
    }
    match receipt.verification_status {
        Some(ReplayVerificationStatus::Verified) => Some(MatchExitState::Confirmed),
        Some(ReplayVerificationStatus::Rejected) => Some(MatchExitState::Rejected),
        _ => None,
    }
}

fn committed_terminal(
    state: &AppState,
    account: &sow_data::db::PlayerAccount,
    match_id: &str,
) -> WorkResult<Option<MatchExitState>> {
    let receipt = account.profile.reward_receipts.get(match_id);
    let phase = terminal(receipt);
    if phase.is_some() {
        if !state.db.pending_match_progress(match_id)?.is_empty() {
            let persisted = match receipt.and_then(|receipt| receipt.personal_closure_turn) {
                Some(turn) => marker_matches(&participant_result_path(match_id,&account.id,turn),&format!("{match_id}:{}:{turn}",account.id))?,
                None => marker_matches(&match_result_path(match_id),match_id)?,
            };
            if !persisted {
                return Ok(None);
            }
        }
        if state
            .db
            .match_work(&format!("participant:{match_id}:{}", account.id))?
            .is_some()
        {
            return Ok(None);
        }
        if receipt.is_some_and(|receipt| receipt.personal_closure_turn.is_none())
            && state.db.match_work(match_id)?.is_some()
        {
            return Ok(None);
        }
    }
    Ok(phase)
}

fn wake(state: &AppState, key: String) {
    if let Err(error) = state.replay_verification_tx.try_send(key) {
        warn!("[rewards] durable work notification deferred: {error}");
    }
}

#[derive(Deserialize)]
pub(super) struct ExitRequest {
    account_id: String,
    match_id: String,
    #[serde(default)]
    auth_secret: Option<String>,
    #[serde(default)]
    closure_known: bool,
}

/// Authenticated observation never creates an exit command. Subscribe before
/// reading durable state so a completion between the read and wait is retained.
pub(super) async fn handle_result(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<ExitRequest>,
) -> axum::response::Response {
    let account_id = match store_account_for_request(
        &state,
        &headers,
        &request.account_id,
        request.auth_secret.as_deref(),
    )
    .await
    {
        Ok(account_id) => account_id,
        Err(response) => return response.into_response(),
    };
    if !valid_match_participant_settlement_params(&request.match_id, &account_id) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":"invalid match id"})),
        )
            .into_response();
    }
    let mut changes = state.match_results.subscribe();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    let result = async {
        state.db.registered_human_participant(&request.match_id, &account_id)?;
        loop {
            let account = state.db.account_by_id(&account_id).await?;
            if let Some(phase) = committed_terminal(&state, &account, &request.match_id)? {
                return Ok::<_, Box<dyn std::error::Error + Send + Sync>>((StatusCode::OK, serde_json::json!({
                    "account_id":account.id, "match_id":request.match_id, "state":phase,
                    "display_name":account.display_name, "profile":account.profile,
                })));
            }
            if !request.closure_known {
                let personal = state.db.match_work(&format!("participant:{}:{account_id}", request.match_id))?;
                if personal.is_some() || state.db.match_work(&request.match_id)?.is_some() {
                    return Ok((StatusCode::ACCEPTED, serde_json::json!({
                        "account_id":account_id, "match_id":request.match_id,
                        "state":personal.map_or(MatchExitState::AwaitingResult, |job| job.state),
                        "closure_recorded":true,
                    })));
                }
            }
            let timed_out = loop {
                match tokio::time::timeout_at(deadline, changes.recv()).await {
                    Ok(Ok(match_id)) if match_id == request.match_id => break false,
                    Ok(Ok(_)) => continue,
                    Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(count))) => {
                        warn!("[rewards] result observer missed {count} wakeups; rereading durable state");
                        break false;
                    }
                    Ok(Err(error)) => return Err(error.into()),
                    Err(_) => break true,
                }
            };
            if timed_out {
                let job = state.db.match_work(&format!("participant:{}:{account_id}", request.match_id))?;
                let phase = job.as_ref().map_or(MatchExitState::AwaitingResult, |job| job.state);
                return Ok((StatusCode::ACCEPTED, serde_json::json!({
                    "account_id":account_id, "match_id":request.match_id, "state":phase,
                })));
            }
        }
    }.await;
    match result {
        Ok((code, body)) => (code, Json(body)).into_response(),
        Err(error) => {
            warn!(
                "[rewards] result observation match={} account={}: {error}",
                request.match_id,
                account_hint(Some(&account_id))
            );
            (
                participant_closure_error_status(&error.to_string()),
                Json(ErrorResponse {
                    error: error.to_string(),
                }),
            )
                .into_response()
        }
    }
}

pub(super) async fn handle_exit(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<ExitRequest>,
) -> axum::response::Response {
    let account_id = match store_account_for_request(
        &state,
        &headers,
        &request.account_id,
        request.auth_secret.as_deref(),
    )
    .await
    {
        Ok(account_id) => account_id,
        Err(response) => return response.into_response(),
    };
    if !valid_match_participant_settlement_params(&request.match_id, &account_id) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":"invalid match id"})),
        )
            .into_response();
    }
    let result = async {
        state
            .db
            .registered_human_participant(&request.match_id, &account_id)?;
        let account = state.db.account_by_id(&account_id).await?;
        let work = MatchWork::RecoveryExit {
            match_id: request.match_id.clone(),
            account_id: account_id.clone(),
        };
        let (phase, retryable_error) =
            if let Some(phase) = committed_terminal(&state, &account, &request.match_id)? {
                (phase, false)
            } else {
                let record = state.db.queue_match_work(work.clone())?;
                // A notification is only a wakeup; the committed queue owns recovery.
                wake(&state, work.key());
                let participant_key = format!("participant:{}:{}", request.match_id, account_id);
                let phase = match state.db.match_work(&participant_key)? {
                    Some(participant) if participant.state == MatchExitState::Verifying => {
                        MatchExitState::Verifying
                    }
                    Some(_) => MatchExitState::AwaitingResult,
                    None => record.state,
                };
                (phase, record.last_error.is_some())
            };
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>((phase, retryable_error, account))
    }
    .await;
    match result {
        Ok((phase, retryable_error, account)) => {
            let code = if matches!(phase, MatchExitState::Confirmed | MatchExitState::Rejected) {
                StatusCode::OK
            } else {
                StatusCode::ACCEPTED
            };
            (code, Json(serde_json::json!({
                "account_id": account.id, "match_id": request.match_id, "state": phase,
                "display_name": account.display_name, "profile": account.profile, "retryable_error": retryable_error,
            }))).into_response()
        }
        Err(error) => {
            let message = error.to_string();
            warn!(
                "[rewards] exit request rejected match={} account={} reason={message}",
                request.match_id,
                account_hint(Some(&account_id))
            );
            (
                participant_closure_error_status(&message),
                Json(ErrorResponse { error: message }),
            )
                .into_response()
        }
    }
}

/// Only the relay may enqueue a personal replay. ACK follows durable bytes
/// and durable work, not expensive verification or the life of a game port.
pub(super) async fn accept_participant(
    state: &AppState,
    match_id: &str,
    account_id: &str,
    closure_turn: u64,
    body: Bytes,
) -> WorkResult<serde_json::Value> {
    let intake_started = Instant::now();
    state
        .db
        .registered_human_participant(match_id, account_id)?;
    let _guard = state.match_artifact_lock.lock().await;
    let account = state.db.account_by_id(account_id).await?;
    let receipt = account.profile.reward_receipts.get(match_id);
    if terminal(receipt).is_some() {
        return Ok(
            serde_json::json!({"status":"settled", "match_id":match_id, "account_id":account_id, "closure_turn":closure_turn,
                "settled_closure_turn":receipt.and_then(|receipt| receipt.personal_closure_turn)}),
        );
    }
    // An already accepted whole replay owns recovery if its relay copy was
    // removed between the exit command's lookup and delivery. A missing local
    // copy cannot reject a result that the database is already checking.
    if body.is_empty() && state.db.match_work(match_id)?.is_some() {
        return Ok(
            serde_json::json!({"status":"queued", "match_id":match_id,"account_id":account_id,"closure_turn":closure_turn}),
        );
    }
    let work = MatchWork::Participant {
        match_id: match_id.to_string(),
        account_id: account_id.to_string(),
        closure_turn,
        replay_sha256: hex::encode(Sha256::digest(&body)),
        rejected: body.is_empty(),
    };
    let key = work.key();
    if let Some(existing) = state.db.match_work(&key)? {
        if existing.work != work {
            return Err("pending result conflicts with its saved state".into());
        }
    } else {
        let path = participant_path(match_id, account_id);
        let parent = path
            .parent()
            .ok_or("participant replay directory unavailable")?;
        tokio::fs::create_dir_all(parent).await?;
        let temporary = path.with_extension("replay.tmp");
        let mut file = tokio::fs::File::create(&temporary).await?;
        file.write_all(&body).await?;
        file.sync_all().await?;
        drop(file);
        tokio::fs::rename(&temporary, &path).await?;
        std::fs::File::open(parent)?.sync_all()?;
        state.db.queue_match_work(work)?;
    }
    wake(state, key);
    let _ = state.match_results.send(match_id.to_string());
    info!(
        "[exit-timing] accepted match={match_id} account={} turn={closure_turn} intake_ms={}",
        account_hint(Some(account_id)),
        intake_started.elapsed().as_millis()
    );
    Ok(
        serde_json::json!({"status":"queued", "match_id":match_id, "account_id":account_id, "closure_turn":closure_turn}),
    )
}

async fn dispatch_recovery_exit(state: &AppState, work: &MatchWork) -> WorkResult<bool> {
    let MatchWork::RecoveryExit {
        match_id,
        account_id,
    } = work
    else {
        return Err("invalid exit work".into());
    };
    let account = state.db.account_by_id(account_id).await?;
    if committed_terminal(state, &account, match_id)?.is_some() {
        return Ok(true);
    }
    if state
        .db
        .match_work(&format!("participant:{match_id}:{account_id}"))?
        .is_some()
        || state.db.match_work(match_id)?.is_some()
    {
        return Ok(false);
    }
    for sequence in state.db.pending_match_progress(match_id)? {
        let bytes = tokio::fs::read(existing_progress_path(match_id,sequence)).await?;
        let (batch, _) = decode_progress(&bytes)?;
        if batch.finished || batch.participant_closures.iter().any(|c| c.account_id == *account_id) { return Ok(false); }
    }
    let base = std::env::var("SOW_SERVER_HTTP_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:25566".to_string());
    let response = state
        .replay_verification_client
        .post(format!(
            "{}/internal/match/exit",
            base.trim_end_matches('/')
        ))
        .bearer_auth(&state.secret_token)
        .timeout(Duration::from_secs(10))
        .json(&serde_json::json!({"match_id":match_id,"account_id":account_id}))
        .send()
        .await?;
    let status = response.status();
    let body: serde_json::Value = response.json().await?;
    if !status.is_success() {
        return Err(format!("exit control HTTP {status}: {body}").into());
    }
    if body.get("match_id").and_then(serde_json::Value::as_str) != Some(match_id)
        || body.get("account_id").and_then(serde_json::Value::as_str) != Some(account_id)
        || !matches!(
            body.get("status").and_then(serde_json::Value::as_str),
            Some("recorded" | "awaiting_result")
        )
    {
        return Err("exit control response did not confirm this participant".into());
    }
    Ok(false)
}

async fn verify_participant(state: &AppState, work: &MatchWork) -> WorkResult<bool> {
    let MatchWork::Participant {
        match_id,
        account_id,
        closure_turn,
        replay_sha256,
        rejected,
    } = work
    else {
        return Err("invalid participant work".into());
    };
    let account = state.db.account_by_id(account_id).await?;
    let receipt = account.profile.reward_receipts.get(match_id);
    if terminal(receipt).is_some()
        && receipt.and_then(|r| r.personal_closure_turn) != Some(*closure_turn)
    {
        // A whole result may have completed while this personal result was
        // waiting. Its durable job still owns any unfinished profile writes.
        return Ok(state.db.match_work(match_id)?.is_none());
    }
    // A matching receipt in Valkey can precede the checked REDB/history save.
    // Keep processing this durable job until the existing apply operation
    // finishes both; that operation already prevents a second reward.
    let bytes = match tokio::fs::read(participant_path(match_id, account_id)).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            warn!(
                "[rewards] accepted participant artifact is missing match={match_id}; recording explicit rejection"
            );
            reject_participant_settlement(&state.db, match_id, account_id, *closure_turn).await?;
            return Ok(true);
        }
        Err(error) => return Err(error.into()),
    };
    if *rejected || hex::encode(Sha256::digest(&bytes)) != *replay_sha256 {
        reject_participant_settlement(&state.db, match_id, account_id, *closure_turn).await?;
        return Ok(true);
    }
    let base = std::env::var("SOW_SERVER_HTTP_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:25566".to_string());
    let url = format!(
        "{}/internal/replay/verify/{match_id}/participant/{account_id}/{closure_turn}",
        base.trim_end_matches('/')
    );
    let verify_started = Instant::now();
    let response = state
        .replay_verification_client
        .post(url)
        .bearer_auth(&state.secret_token)
        .header("Content-Type", "application/octet-stream")
        .body(bytes)
        .send()
        .await?;
    if response.status() == reqwest::StatusCode::ACCEPTED { return Ok(false); }
    if response.status() == reqwest::StatusCode::UNPROCESSABLE_ENTITY {
        reject_participant_settlement(&state.db, match_id, account_id, *closure_turn).await?;
        return Ok(true);
    }
    if !response.status().is_success() {
        let status = response.status();
        let reason = response.text().await?;
        return Err(format!("participant verifier HTTP {status}: {reason}").into());
    }
    let result = response
        .json::<sow_data::profile::VerifiedMatchResult>()
        .await?;
    if result.match_id != *match_id || result.personal_closure_turn != Some(*closure_turn) {
        return Err("participant verifier returned a different closure".into());
    }
    let verify_ms = verify_started.elapsed().as_millis();
    let save_started = Instant::now();
    state
        .db
        .note_match_participant_closure(match_id, account_id, *closure_turn)
        .await?;
    let applied = state
        .db
        .apply_verified_match_participant_result(&result, account_id, *closure_turn)
        .await?;
    if applied.is_some() {
        info!(
            "[exit-timing] saved match={match_id} account={} verify_ms={verify_ms} save_ms={}",
            account_hint(Some(account_id)),
            save_started.elapsed().as_millis()
        );
        info!(
            "[rewards] participant receipt verified match={match_id} account={}",
            account_hint(Some(account_id))
        );
    }
    Ok(true)
}

/// One verifier consumes the existing durable queue. Exit delivery has its
/// own bounded in-flight set, so a long replay never blocks another exit.
pub(super) async fn worker(
    state: Arc<AppState>,
    mut notifications: tokio::sync::mpsc::Receiver<String>,
) {
    let (done_tx, mut done_rx) =
        tokio::sync::mpsc::unbounded_channel::<(MatchWorkRecord, WorkResult<bool>)>();
    let mut exits = HashSet::new();
    let mut archives = HashSet::new();
    let mut verifier: Option<String> = None;
    let mut closure_streak = 0u8;
    let mut retry = tokio::time::interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            _ = retry.tick() => {},
            Some(_) = notifications.recv() => {},
            Some((mut job, result)) = done_rx.recv() => {
                let key = job.work.key();
                if matches!(job.work, MatchWork::RecoveryExit { .. }) { exits.remove(&key); }
                else if matches!(job.work, MatchWork::Archive { .. }) { archives.remove(&key); }
                else { verifier = None; }
                match result {
                    Ok(true) => {
                        match state.db.remove_match_work(&key) {
                            Ok(()) => { let _ = state.match_results.send(job.work.match_id().to_string()); }
                            Err(error) => error!("[rewards] completed work {key} could not be removed: {error}"),
                        }
                    }
                    Ok(false) => {
                        job.state = MatchExitState::AwaitingResult;
                        job.retry_at = if matches!(job.work, MatchWork::RecoveryExit { .. }) { now() + 5 } else { 0 };
                        job.queued_at_ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
                        job.last_error = None;
                        if let Err(error) = state.db.save_match_work(&job) { error!("[rewards] work {key} state save failed: {error}"); }
                    }
                    Err(error) => {
                        warn!("[rewards] work {key} remains pending: {error}");
                        job.last_error = Some(error.to_string());
                        job.retry_at = now() + 5;
                        if let Err(error) = state.db.save_match_work(&job) { error!("[rewards] work {key} retry save failed: {error}"); }
                    }
                }
            }
        }
        let mut jobs = match state.db.pending_match_work(now()) {
            Ok(jobs) => jobs,
            Err(error) => {
                error!("[rewards] durable queue read failed: {error}");
                continue;
            }
        };
        // Closures get four slices, then the oldest active match gets one.
        if closure_streak >= 4 {
            if let Some(index) = jobs.iter().position(|job| matches!(job.work, MatchWork::Progress { .. }) && !job.urgent) {
                let progress = jobs.remove(index);
                jobs.insert(0, progress);
            }
            closure_streak = 0;
        }
        for mut job in jobs {
            let key = job.work.key();
            let is_exit = matches!(job.work, MatchWork::RecoveryExit { .. });
            let is_archive = matches!(job.work, MatchWork::Archive { .. });
            if is_exit {
                if exits.contains(&key) || exits.len() >= 8 {
                    continue;
                }
                exits.insert(key.clone());
            } else if is_archive {
                if archives.contains(&key) || archives.len() >= 1 { continue; }
                archives.insert(key.clone());
            } else {
                if verifier.is_some() {
                    continue;
                }
                verifier = Some(key.clone());
                if !matches!(job.work, MatchWork::Progress { .. } | MatchWork::Archive { .. }) || job.urgent { closure_streak = closure_streak.saturating_add(1); }
                job.state = MatchExitState::Verifying;
                if let Err(error) = state.db.save_match_work(&job) {
                    error!("[rewards] work {key} cannot enter verification: {error}");
                    verifier = None;
                    continue;
                }
            }
            let state = Arc::clone(&state);
            let done = done_tx.clone();
            tokio::spawn(async move {
                let current_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                info!(
                    "[exit-timing] work={key} queue_wait_ms={}",
                    if job.queued_at_ms == 0 {
                        0
                    } else {
                        current_ms.saturating_sub(job.queued_at_ms)
                    }
                );
                info!("[rewards] work {key} starting");
                let result = match &job.work {
                    MatchWork::RecoveryExit { .. } => {
                        dispatch_recovery_exit(&state, &job.work).await
                    }
                    MatchWork::Participant { .. } => verify_participant(&state, &job.work).await,
                    MatchWork::Progress { .. } => verify_progress(&state, &job.work).await,
                    MatchWork::Archive { .. } => archive_progress_work(&state,&job.work).await,
                    MatchWork::Replay { match_id } => {
                        verify_queued_replay(&state, match_id).await;
                        match state.db.match_work(match_id) {
                            Ok(None) => Ok(true),
                            Ok(Some(_)) => {
                                Err("whole replay remains pending; see verifier reason".into())
                            }
                            Err(error) => Err(error),
                        }
                    }
                };
                let _ = done.send((job, result));
            });
        }
    }
}

#[cfg(test)]
mod progress_rejection_tests {
    use super::{invalid_verified_result, permanent_progress_rejection};

    #[test]
    fn only_deterministic_verifier_failures_reject_a_progress_stream() {
        assert!(permanent_progress_rejection(reqwest::StatusCode::BAD_REQUEST));
        assert!(permanent_progress_rejection(reqwest::StatusCode::UNPROCESSABLE_ENTITY));
        assert!(!permanent_progress_rejection(reqwest::StatusCode::CONFLICT));
        assert!(!permanent_progress_rejection(reqwest::StatusCode::UNAUTHORIZED));
        assert!(!permanent_progress_rejection(reqwest::StatusCode::SERVICE_UNAVAILABLE));
    }

    #[test]
    fn only_verified_result_contract_errors_are_terminal() {
        assert!(invalid_verified_result(&std::io::Error::other(
            "invalid verified result: winner is not in the registered roster"
        )));
        assert!(!invalid_verified_result(&std::io::Error::other(
            "database connection reset"
        )));
    }
}
