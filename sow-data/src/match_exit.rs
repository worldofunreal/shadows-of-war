//! Server-owned match progress and authoritative reward observation.
use super::*;
use sha2::{Digest, Sha256};
use sow_data::profile::{
    MatchExitState, MatchWork, MatchWorkRecord, ReplayVerificationStatus, RewardSettlementStatus,
};
use std::path::PathBuf;

type WorkResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Serialize, Deserialize)]
struct ProgressIndex {
    match_id: String,
    chunks: Vec<ProgressIndexChunk>,
}

#[derive(Serialize, Deserialize)]
struct ProgressIndexChunk {
    sequence: u64,
    first_turn: u64,
    turn_count: u64,
    previous_sha256: String,
    sha256: String,
    finished: bool,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn replay_dir() -> PathBuf {
    PathBuf::from(std::env::var("SOW_REPLAY_DIR").unwrap_or_else(|_| "replays".to_string()))
}

fn progress_path(match_id: &str, sequence: u64) -> PathBuf {
    progress_path_in(&replay_dir(), match_id, sequence)
}

fn progress_path_in(root: &std::path::Path, match_id: &str, sequence: u64) -> PathBuf {
    root.join(format!("{match_id}.progress.{sequence:010}.bin"))
}

fn decode_progress(bytes: &[u8]) -> WorkResult<sow_data::profile::MatchProgress> {
    bincode::deserialize(bytes).map_err(Into::into)
}

fn progress_index_path(match_id: &str) -> PathBuf {
    progress_path(match_id, 0).with_file_name(format!("{match_id}.progress.index"))
}

async fn read_progress_index(match_id: &str) -> WorkResult<Option<ProgressIndex>> {
    match tokio::fs::read(progress_index_path(match_id)).await {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn closure_result_path(match_id: &str, account_id: &str, turn: u64) -> PathBuf {
    replay_dir().join(format!("{match_id}-{account_id}.closure.{turn}.paid"))
}

fn match_result_path(match_id: &str) -> PathBuf {
    progress_path(match_id, 0).with_file_name(format!("{match_id}.result.paid"))
}

async fn durable_marker(path: &std::path::Path, value: &str) -> WorkResult<()> {
    if tokio::fs::read_to_string(path)
        .await
        .is_ok_and(|saved| saved == value)
    {
        return Ok(());
    }
    let temporary = path.with_extension(format!("{}.tmp", rand::random::<u64>()));
    let mut file = tokio::fs::File::create(&temporary).await?;
    file.write_all(value.as_bytes()).await?;
    file.sync_all().await?;
    drop(file);
    tokio::fs::rename(&temporary, path).await?;
    std::fs::File::open(path.parent().ok_or("result marker directory missing")?)?.sync_all()?;
    Ok(())
}

fn marker_matches(path: &std::path::Path, value: &str) -> WorkResult<bool> {
    match std::fs::read_to_string(path) {
        Ok(saved) => Ok(saved == value),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

async fn archived_chunk_matches(match_id: &str, sequence: u64, sha256: &str) -> WorkResult<bool> {
    let Some(index) = read_progress_index(match_id).await? else {
        return Ok(false);
    };
    if index.match_id != match_id
        || !index
            .chunks
            .iter()
            .any(|chunk| chunk.sequence == sequence && chunk.sha256 == sha256)
    {
        return Ok(false);
    }
    let Some(last) = index.chunks.last() else {
        return Ok(false);
    };
    marker_matches(
        &progress_path(match_id, last.sequence).with_extension("archived"),
        &last.sha256,
    )
}

async fn progress_index(match_id: &str, last_sequence: u64) -> WorkResult<ProgressIndex> {
    let mut chunks = Vec::with_capacity(last_sequence as usize + 1);
    let mut first_turn = 0u64;
    let mut previous = String::new();
    for sequence in 0..=last_sequence {
        let path = progress_path(match_id, sequence);
        let batch = decode_progress(&tokio::fs::read(path).await?)?;
        if batch.match_id != match_id
            || batch.sequence != sequence
            || batch.first_turn != first_turn
            || batch.previous_sha256 != previous
            || batch.replay_data.len() < 8
            || hex::encode(Sha256::digest(batch.signing_bytes()?)) != batch.sha256
            || (sequence < last_sequence && batch.finished)
        {
            return Err("cannot archive a broken turn stream sequence".into());
        }
        let count = u64::from_le_bytes(batch.replay_data[..8].try_into().unwrap());
        chunks.push(ProgressIndexChunk {
            sequence,
            first_turn,
            turn_count: count,
            previous_sha256: previous,
            sha256: batch.sha256.clone(),
            finished: batch.finished,
        });
        first_turn = first_turn.saturating_add(count);
        previous = batch.sha256;
    }
    if !chunks.last().is_some_and(|chunk| chunk.finished) {
        return Err("cannot archive an unfinished turn stream".into());
    }
    Ok(ProgressIndex {
        match_id: match_id.to_string(),
        chunks,
    })
}

async fn prune_progress_chunks(match_id: &str, last_sequence: u64) -> WorkResult<()> {
    let final_marker = progress_path(match_id, last_sequence).with_extension("archived");
    let directory = progress_path(match_id, 0)
        .parent()
        .ok_or("progress directory missing")?
        .to_path_buf();
    let prefix = format!("{match_id}.progress.");
    let mut entries = tokio::fs::read_dir(&directory).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if !name.starts_with(&prefix) || path == final_marker {
            continue;
        }
        let suffix = &name[prefix.len()..];
        if suffix.ends_with(".bin")
            || suffix.ends_with(".json")
            || suffix.ends_with(".done")
            || suffix.ends_with(".tmp")
            || suffix.ends_with(".archived")
        {
            tokio::fs::remove_file(path).await?;
        }
    }
    std::fs::File::open(directory)?.sync_all()?;
    Ok(())
}

pub(super) async fn handle_relay_progress(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> axum::response::Response {
    if !verify_internal_auth(&headers, &state.relay_settlement_secret) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let incoming: sow_data::profile::RelayMatchProgress = match bincode::deserialize(&body) {
        Ok(batch) => batch,
        Err(error) => {
            warn!("[rewards] invalid relay progress request: {error}");
            return StatusCode::BAD_REQUEST.into_response();
        }
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
        store_progress_locked(&state, batch, count).await
    }
    .await;
    match result {
        Ok(ack) => (StatusCode::ACCEPTED, Json(ack)).into_response(),
        Err(error) => {
            warn!(
                "[rewards] relay progress intake pending match={}: {error}",
                incoming.match_id
            );
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: error.to_string(),
                }),
            )
                .into_response()
        }
    }
}

async fn store_progress_locked(
    state: &AppState,
    batch: sow_data::profile::MatchProgress,
    count: u64,
) -> WorkResult<serde_json::Value> {
    if batch.match_id.parse::<u64>().is_err()
        || batch.replay_data.len() > sow_data::MAX_MATCH_CHUNK_BYTES
        || batch.replay_data.len() < 8
        || hex::encode(Sha256::digest(batch.signing_bytes()?)) != batch.sha256
    {
        return Err("invalid turn stream delivery".into());
    }
    let start = state
        .db
        .match_start_record(&batch.match_id)?
        .ok_or("trusted match registration unavailable")?;
    let tick_rate = start
        .metadata
        .pointer("/config/tick_rate_ms")
        .and_then(serde_json::Value::as_f64)
        .ok_or("trusted clock missing")? as f32;
    if !tick_rate.is_finite()
        || tick_rate <= 0.0
        || count > sow_data::max_match_turns(tick_rate)
        || batch.first_turn.saturating_add(count) > sow_data::max_match_turns(tick_rate)
    {
        return Err("invalid turn stream duration".into());
    }
    let path = progress_path(&batch.match_id, batch.sequence);
    let existing = tokio::fs::read(&path).await;
    match existing {
        Ok(bytes) => {
            let saved = decode_progress(&bytes)?;
            if saved != batch {
                return Err("turn stream delivery conflicts with saved state".into());
            }
            if !marker_matches(&path.with_extension("done"), &saved.sha256)? {
                let job = state.db.queue_match_work(progress_work(&batch))?;
                wake(state, job.work.key());
            } else if batch.finished
                && !marker_matches(&path.with_extension("archived"), &batch.sha256)?
            {
                let job = state.db.queue_match_work(archive_work(&batch))?;
                wake(state, job.work.key());
            }
            return Ok(
                serde_json::json!({"match_id":batch.match_id,"sequence":batch.sequence,"sha256":batch.sha256}),
            );
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if batch.finished && marker_matches(&path.with_extension("archived"), &batch.sha256)? {
                return Ok(
                    serde_json::json!({"match_id":batch.match_id,"sequence":batch.sequence,"sha256":batch.sha256}),
                );
            }
            if let Some(index) = read_progress_index(&batch.match_id).await? {
                let saved = index.match_id == batch.match_id
                    && index.chunks.iter().any(|chunk| {
                        chunk.sequence == batch.sequence
                            && chunk.first_turn == batch.first_turn
                            && chunk.previous_sha256 == batch.previous_sha256
                            && chunk.sha256 == batch.sha256
                            && chunk.finished == batch.finished
                    });
                let archived = if let Some(chunk) = index.chunks.last() {
                    marker_matches(
                        &progress_path(&batch.match_id, chunk.sequence).with_extension("archived"),
                        &chunk.sha256,
                    )?
                } else {
                    false
                };
                if saved && archived {
                    return Ok(
                        serde_json::json!({"match_id":batch.match_id,"sequence":batch.sequence,"sha256":batch.sha256}),
                    );
                }
                return Err("turn stream delivery conflicts with its archived record".into());
            }
        }
        Err(e) => return Err(e.into()),
    }
    if batch.sequence == 0 {
        if batch.first_turn != 0 || !batch.previous_sha256.is_empty() {
            return Err("turn stream must start at turn zero".into());
        }
    } else {
        let bytes = tokio::fs::read(progress_path(&batch.match_id, batch.sequence - 1)).await?;
        let previous = decode_progress(&bytes)?;
        let previous_count = u64::from_le_bytes(previous.replay_data[..8].try_into().unwrap());
        if previous.sha256 != batch.previous_sha256
            || previous.first_turn + previous_count != batch.first_turn
            || previous.finished
        {
            return Err("turn stream delivery conflicts with saved sequence".into());
        }
    }
    for closure in &batch.participant_closures {
        state
            .db
            .registered_human_participant(&batch.match_id, &closure.account_id)?;
        if closure.turn_number >= batch.first_turn + count
            || !matches!(
                closure.reason.as_str(),
                "voluntary" | "timeout" | "match_ended" | "defeated"
            )
        {
            return Err("invalid turn stream closure".into());
        }
    }
    let parent = path.parent().ok_or("turn stream directory missing")?;
    tokio::fs::create_dir_all(parent).await?;
    let temporary = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("bin")
    ));
    let mut file = tokio::fs::File::create(&temporary).await?;
    let encoded = bincode::serialize(&batch)?;
    file.write_all(&encoded).await?;
    file.sync_all().await?;
    drop(file);
    tokio::fs::rename(temporary, &path).await?;
    std::fs::File::open(parent)?.sync_all()?;
    let job = state.db.queue_match_work(progress_work(&batch))?;
    wake(state, job.work.key());
    Ok(
        serde_json::json!({"match_id":batch.match_id,"sequence":batch.sequence,"sha256":batch.sha256}),
    )
}

async fn previous_progress_hash(match_id: &str, sequence: u64) -> WorkResult<String> {
    if sequence == 0 {
        return Ok(String::new());
    }
    if let Some(index) = read_progress_index(match_id).await? {
        if index.match_id != match_id {
            return Err("progress index belongs to another match".into());
        }
        if let Some(previous) = index.chunks.get(sequence.saturating_sub(1) as usize) {
            return Ok(previous.sha256.clone());
        }
    }
    let bytes = tokio::fs::read(progress_path(match_id, sequence - 1)).await?;
    let previous = decode_progress(&bytes)?;
    Ok(previous.sha256)
}

async fn reject_relay_recording(
    state: &AppState,
    incoming: &sow_data::profile::RelayMatchProgress,
) -> WorkResult<serde_json::Value> {
    let start = state
        .db
        .match_start_record(&incoming.match_id)?
        .ok_or("trusted match registration unavailable")?;
    for closure in &incoming.participant_closures {
        state
            .db
            .registered_human_participant(&incoming.match_id, &closure.account_id)?;
        if closure.turn_number
            > sow_data::max_match_turns(
                start
                    .metadata
                    .pointer("/config/tick_rate_ms")
                    .and_then(serde_json::Value::as_f64)
                    .ok_or("trusted clock missing")? as f32,
            )
            || !matches!(
                closure.reason.as_str(),
                "voluntary" | "timeout" | "match_ended" | "defeated"
            )
        {
            return Err("invalid failed-recording closure".into());
        }
    }
    if start.replay_status == ReplayVerificationStatus::Pending {
        if incoming.finished {
            state
                .db
                .finalize_match_with_lobby(&incoming.match_id, None)
                .await?;
        }
        super::reject_match_verification(state, &incoming.match_id).await?;
    }
    settle_rejected_progress_closures(
        state,
        &sow_data::profile::MatchProgress {
            match_id: incoming.match_id.clone(),
            sequence: incoming.sequence,
            first_turn: incoming.first_turn,
            previous_sha256: String::new(),
            sha256: String::new(),
            replay_data: Vec::new(),
            participant_closures: incoming.participant_closures.clone(),
            finished: incoming.finished,
        },
        &[],
    )
    .await?;
    durable_marker(&match_result_path(&incoming.match_id), &incoming.match_id).await?;
    let _ = state.match_results.send(incoming.match_id.clone());
    Ok(
        serde_json::json!({"match_id":incoming.match_id,"sequence":incoming.sequence,"recording_failed":true}),
    )
}

fn progress_work(batch: &sow_data::profile::MatchProgress) -> MatchWork {
    MatchWork::Progress {
        match_id: batch.match_id.clone(),
        sequence: batch.sequence,
        sha256: batch.sha256.clone(),
        closing: batch.finished || !batch.participant_closures.is_empty(),
    }
}

fn archive_work(batch: &sow_data::profile::MatchProgress) -> MatchWork {
    MatchWork::Archive {
        match_id: batch.match_id.clone(),
        sequence: batch.sequence,
        sha256: batch.sha256.clone(),
    }
}

fn permanent_progress_rejection(status: reqwest::StatusCode) -> bool {
    matches!(
        status,
        reqwest::StatusCode::BAD_REQUEST | reqwest::StatusCode::UNPROCESSABLE_ENTITY
    )
}

fn invalid_verified_result(error: &dyn std::error::Error) -> bool {
    error.to_string().starts_with("invalid verified result:")
}

#[derive(Deserialize)]
struct VerifierHistoryRequired {
    expected_sequence: u64,
    verified_turns: u64,
}

async fn complete_progress_chunk(
    state: &AppState,
    batch: &sow_data::profile::MatchProgress,
) -> WorkResult<bool> {
    durable_marker(
        &progress_path(&batch.match_id, batch.sequence).with_extension("done"),
        &batch.sha256,
    )
    .await?;
    if batch.finished {
        let job = state.db.queue_match_work(archive_work(batch))?;
        wake(state, job.work.key());
    }
    Ok(true)
}

async fn settle_rejected_progress_closures(
    state: &AppState,
    batch: &sow_data::profile::MatchProgress,
    additional: &[(String, u64)],
) -> WorkResult<()> {
    let mut closures = batch
        .participant_closures
        .iter()
        .map(|closure| (closure.account_id.clone(), closure.turn_number))
        .chain(additional.iter().cloned())
        .collect::<Vec<_>>();
    closures.sort_unstable();
    closures.dedup();
    for (account_id, turn) in closures {
        super::reject_participant_settlement(&state.db, &batch.match_id, &account_id, turn)
            .await
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
        durable_marker(
            &closure_result_path(&batch.match_id, &account_id, turn),
            &format!("{}:{account_id}:{turn}", batch.match_id),
        )
        .await?;
    }
    Ok(())
}

async fn reject_progress(
    state: &AppState,
    batch: &sow_data::profile::MatchProgress,
    additional_closures: &[(String, u64)],
    reason: &str,
) -> WorkResult<bool> {
    super::reject_match_verification(state, &batch.match_id).await?;
    settle_rejected_progress_closures(state, batch, additional_closures).await?;
    durable_marker(&match_result_path(&batch.match_id), &batch.match_id).await?;
    warn!(
        "[rewards] progress permanently rejected match={} sequence={} reason={reason}",
        batch.match_id, batch.sequence
    );
    complete_progress_chunk(state, batch).await
}

async fn verify_progress(state: &AppState, work: &MatchWork) -> WorkResult<bool> {
    let MatchWork::Progress {
        match_id,
        sequence,
        sha256,
        ..
    } = work
    else {
        return Err("invalid turn stream work".into());
    };
    if archived_chunk_matches(match_id, *sequence, sha256).await? {
        return Ok(true);
    }
    let bytes = tokio::fs::read(progress_path(match_id, *sequence)).await?;
    let batch = decode_progress(&bytes)?;
    if &batch.sha256 != sha256 || hex::encode(Sha256::digest(batch.signing_bytes()?)) != *sha256 {
        return Err("saved turn stream checksum mismatch".into());
    }
    let start = state
        .db
        .match_start_record(match_id)?
        .ok_or("trusted match registration unavailable")?;
    match start.replay_status {
        ReplayVerificationStatus::Rejected => {
            settle_rejected_progress_closures(state, &batch, &[]).await?;
            durable_marker(&match_result_path(match_id), match_id).await?;
            return complete_progress_chunk(state, &batch).await;
        }
        ReplayVerificationStatus::Verified => {
            durable_marker(&match_result_path(match_id), match_id).await?;
            return complete_progress_chunk(state, &batch).await;
        }
        ReplayVerificationStatus::Pending => {}
    }
    let base =
        std::env::var("SOW_SERVER_HTTP_URL").unwrap_or_else(|_| "http://127.0.0.1:25566".into());
    let verifier_started = Instant::now();
    let response = state
        .replay_verification_client
        .post(format!(
            "{}/internal/match/progress",
            base.trim_end_matches('/')
        ))
        .bearer_auth(&state.secret_token)
        .header("Content-Type", "application/octet-stream")
        .body(bytes)
        .send()
        .await?;
    info!(
        "[rewards-timing] match={match_id} sequence={sequence} stage=progress_verifier_http elapsed_ms={}",
        verifier_started.elapsed().as_millis()
    );
    if response.status() == reqwest::StatusCode::CONFLICT {
        let required: VerifierHistoryRequired = serde_json::from_str(&response.text().await?)?;
        let first_sequence = required.expected_sequence;
        if first_sequence >= *sequence {
            return Err(format!(
                "verifier history cursor does not precede requested block: expected={first_sequence} requested={sequence}"
            )
            .into());
        }
        let mut recovery = Vec::with_capacity((*sequence - first_sequence) as usize);
        for prior in first_sequence..*sequence {
            let bytes = tokio::fs::read(progress_path(match_id, prior)).await?;
            let saved = decode_progress(&bytes)?;
            recovery.push(progress_work(&saved));
        }
        state.db.queue_match_work_batch(recovery)?;
        info!(
            "[rewards] verifier recovery match={match_id} requeue_from={first_sequence} requested={sequence} verified_turns={}",
            required.verified_turns
        );
        return Ok(false);
    }
    if permanent_progress_rejection(response.status()) {
        let status = response.status();
        let reason = response
            .text()
            .await
            .unwrap_or_else(|error| format!("response unavailable: {error}"));
        return reject_progress(
            state,
            &batch,
            &[],
            &format!("verifier HTTP {status}: {reason}"),
        )
        .await;
    }
    if !response.status().is_success() {
        return Err(format!(
            "turn stream verifier HTTP {}: {}",
            response.status(),
            response.text().await?
        )
        .into());
    }
    let checked = response
        .json::<sow_data::profile::VerifiedProgress>()
        .await?;
    if checked.match_id != *match_id || checked.sequence != *sequence || checked.sha256 != *sha256 {
        return reject_progress(
            state,
            &batch,
            &[],
            "verifier returned another turn stream delivery",
        )
        .await;
    }
    for outcome in &checked.participants {
        if outcome.result.match_id != *match_id {
            return reject_progress(state, &batch, &[], "verifier returned another match").await;
        }
        if outcome.result.personal_closure_turn.is_none() {
            return reject_progress(state, &batch, &[], "verified closure turn missing").await;
        }
    }
    let closures = checked
        .participants
        .iter()
        .filter_map(|outcome| {
            outcome
                .result
                .personal_closure_turn
                .map(|turn| sow_data::profile::ProgressClosure {
                    account_id: outcome.account_id.clone(),
                    turn_number: turn,
                    reason: outcome.reason.clone(),
                })
        })
        .collect::<Vec<_>>();
    let notification = sow_data::profile::MatchResultControl {
        match_id: match_id.clone(),
        terminal_turn: checked.terminal_turn,
        participant_closures: closures.clone(),
    };
    let participant_apply_started = Instant::now();
    if let Err(error) = state
        .db
        .apply_verified_match_participant_results(&checked.participants, Some(notification.clone()))
        .await
    {
        if invalid_verified_result(error.as_ref()) {
            let closures = checked
                .participants
                .iter()
                .filter_map(|item| {
                    item.result
                        .personal_closure_turn
                        .map(|turn| (item.account_id.clone(), turn))
                })
                .collect::<Vec<_>>();
            return reject_progress(state, &batch, &closures, &error.to_string()).await;
        }
        return Err(error);
    }
    info!(
        "[rewards-timing] match={match_id} sequence={sequence} stage=participant_results_apply count={} elapsed_ms={}",
        checked.participants.len(),
        participant_apply_started.elapsed().as_millis()
    );
    for outcome in &checked.participants {
        let turn = outcome
            .result
            .personal_closure_turn
            .expect("validated above");
        durable_marker(
            &closure_result_path(match_id, &outcome.account_id, turn),
            &format!("{match_id}:{}:{turn}", outcome.account_id),
        )
        .await?;
    }
    if let Some(result) = &checked.result {
        let result_apply_started = Instant::now();
        if let Err(error) = state
            .db
            .apply_verified_match_result(result, Some(notification.clone()))
            .await
        {
            if invalid_verified_result(error.as_ref()) {
                let participant_closures = checked
                    .participants
                    .iter()
                    .filter_map(|item| {
                        item.result
                            .personal_closure_turn
                            .map(|turn| (item.account_id.clone(), turn))
                    })
                    .collect::<Vec<_>>();
                return reject_progress(state, &batch, &participant_closures, &error.to_string())
                    .await;
            }
            return Err(error);
        }
        info!(
            "[rewards-timing] match={match_id} sequence={sequence} stage=progress_result_apply elapsed_ms={}",
            result_apply_started.elapsed().as_millis()
        );
    }
    if checked.result.is_some() {
        state
            .db
            .finish_match_verification(match_id, ReplayVerificationStatus::Verified)?;
        durable_marker(&match_result_path(match_id), match_id).await?;
    }
    let _ = state.match_results.send(match_id.clone());
    if checked.complete {
        return complete_progress_chunk(state, &batch).await;
    }
    Ok(false)
}

async fn notify_result(
    state: &AppState,
    command: &sow_data::profile::MatchResultControl,
) -> WorkResult<bool> {
    let base =
        std::env::var("SOW_SERVER_HTTP_URL").unwrap_or_else(|_| "http://127.0.0.1:25566".into());
    let response = state
        .replay_verification_client
        .post(format!(
            "{}/internal/match/result",
            base.trim_end_matches('/')
        ))
        .bearer_auth(&state.secret_token)
        .json(command)
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(format!("verified result delivery HTTP {}", response.status()).into());
    }
    Ok(true)
}

async fn archive_progress(
    state: &AppState,
    last: &sow_data::profile::MatchProgress,
) -> WorkResult<()> {
    use tokio::io::AsyncSeekExt;
    let path = replay_dir().join(format!("{}.replay", last.match_id));
    let temporary = path.with_extension("replay.tmp");
    let mut file = tokio::fs::File::create(&temporary).await?;
    file.write_all(&0u64.to_le_bytes()).await?;
    let mut turns = 0u64;
    let mut archive_bytes = 8u64;
    for sequence in 0..=last.sequence {
        let bytes = tokio::fs::read(progress_path(&last.match_id, sequence)).await?;
        let batch = decode_progress(&bytes)?;
        if batch.replay_data.len() < 8 {
            return Err("progress archive chunk is incomplete".into());
        }
        archive_bytes =
            archive_bytes.saturating_add(batch.replay_data.len().saturating_sub(8) as u64);
        if archive_bytes > sow_data::MAX_MATCH_ARCHIVE_BYTES as u64 {
            return Err("match archive exceeds its registered size limit".into());
        }
        turns += u64::from_le_bytes(batch.replay_data[..8].try_into().unwrap());
        file.write_all(&batch.replay_data[8..]).await?;
    }
    file.seek(std::io::SeekFrom::Start(0)).await?;
    file.write_all(&turns.to_le_bytes()).await?;
    file.sync_all().await?;
    drop(file);
    tokio::fs::rename(temporary, &path).await?;
    let start = state
        .db
        .match_start_record(&last.match_id)?
        .ok_or("trusted match registration missing")?;
    let meta = path.with_extension("json");
    let temporary = meta.with_extension("json.tmp");
    let mut file = tokio::fs::File::create(&temporary).await?;
    file.write_all(&serde_json::to_vec(&start.metadata)?)
        .await?;
    file.sync_all().await?;
    drop(file);
    tokio::fs::rename(temporary, meta).await?;
    std::fs::File::open(path.parent().unwrap())?.sync_all()?;
    Ok(())
}

async fn archive_progress_work(state: &AppState, work: &MatchWork) -> WorkResult<bool> {
    let MatchWork::Archive {
        match_id,
        sequence,
        sha256,
    } = work
    else {
        return Err("invalid archive work".into());
    };
    let artifact = progress_path(match_id, *sequence);
    let archived = artifact.with_extension("archived");
    if marker_matches(&archived, sha256)? {
        let index = read_progress_index(match_id)
            .await?
            .ok_or("archived turn stream index is missing")?;
        if index.match_id != *match_id
            || !index
                .chunks
                .last()
                .is_some_and(|chunk| chunk.sequence == *sequence && chunk.sha256 == *sha256)
        {
            return Err("archived turn stream index conflicts with its final chunk".into());
        }
        prune_progress_chunks(match_id, *sequence).await?;
        return Ok(true);
    }
    let batch = decode_progress(&tokio::fs::read(&artifact).await?)?;
    if !batch.finished
        || batch.sha256 != *sha256
        || hex::encode(Sha256::digest(batch.signing_bytes()?)) != *sha256
    {
        return Err("final archive does not match its durable work".into());
    }
    let index = progress_index(match_id, *sequence).await?;
    archive_progress(state, &batch).await?;
    durable_marker(
        &progress_index_path(match_id),
        &serde_json::to_string(&index)?,
    )
    .await?;
    durable_marker(&archived, sha256).await?;
    prune_progress_chunks(match_id, *sequence).await?;
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

/// A terminal match flag alone cannot authorize retiring its saved engine.
/// Check each registered human's atomic receipt using the same completion rule
/// as player observation. This is read-only and never reconstructs a reward.
pub(super) async fn checkpoint_rewards_terminal(state: &AppState, id: &str) -> WorkResult<bool> {
    let start = state
        .db
        .match_start_record(id)?
        .ok_or("checkpoint registration unavailable")?;
    if start.replay_status == ReplayVerificationStatus::Pending {
        return Ok(false);
    }
    let players = start.metadata["relay_players"]
        .as_array()
        .ok_or("checkpoint roster unavailable")?;
    for player in players
        .iter()
        .filter(|player| !player["is_internal"].as_bool().unwrap_or(false))
    {
        let account_id = player["database_account_id"]
            .as_str()
            .ok_or("checkpoint human account unavailable")?;
        let account = state.db.account_with_events(account_id, &[id]).await?;
        if terminal(account.profile.reward_receipts.get(id)).is_none() {
            return Ok(false);
        }
    }
    Ok(true)
}

fn wake(state: &AppState, key: String) {
    if let Err(error) = state.replay_verification_tx.try_send(key) {
        warn!("[rewards] durable work notification deferred: {error}");
    }
}

#[derive(Deserialize)]
pub(super) struct MatchResultRequest {
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
    Json(request): Json<MatchResultRequest>,
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
        let start = state.db.registered_human_participant(&request.match_id, &account_id)?;
        let ranked = start.metadata.get("kind").and_then(serde_json::Value::as_str)
            == Some("Ranked1v1");
        loop {
            let account = state.db.account_with_events(&account_id, &[&request.match_id]).await?;
            if let Some(phase) = terminal(account.profile.reward_receipts.get(&request.match_id)) {
                let ranked_result = if ranked {
                    state.db.ranked_match_result(&account_id, &request.match_id)?
                } else {
                    None
                };
                if ranked && phase == MatchExitState::Confirmed && ranked_result.is_none() {
                    // A participant can collect an early exit receipt while the duel is still
                    // running. Ranked return waits for the verified result of both players.
                } else {
                return Ok::<_, Box<dyn std::error::Error + Send + Sync>>((StatusCode::OK, serde_json::json!({
                    "account_id":account.id, "match_id":request.match_id, "state":phase,
                    "display_name":account.display_name, "profile":account.profile,
                    "ranked_result":ranked_result,
                })));
                }
            }
            if !request.closure_known {
                if let Some(work) = state.db.match_work(&request.match_id)? {
                    return Ok((StatusCode::ACCEPTED, serde_json::json!({
                        "account_id":account_id, "match_id":request.match_id,
                        "state":work.state,
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
                let job = state.db.match_work(&request.match_id)?;
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

pub(super) async fn worker(
    state: Arc<AppState>,
    mut notifications: tokio::sync::mpsc::Receiver<String>,
) {
    let (done_tx, mut done_rx) =
        tokio::sync::mpsc::unbounded_channel::<(MatchWorkRecord, WorkResult<bool>)>();
    let mut archives = HashSet::new();
    let mut notifications_in_flight = HashSet::new();
    let mut verifiers = HashSet::new();
    let mut confirmed_notifications_pruned = false;
    let parallelism =
        std::thread::available_parallelism().map_or(1, |n| n.get().saturating_sub(1).max(1));
    let mut closure_streak = 0u8;
    let mut retry = tokio::time::interval(Duration::from_secs(1));
    loop {
        if !confirmed_notifications_pruned {
            match state.db.remove_confirmed_match_notifications() {
                Ok(removed) => {
                    if removed > 0 {
                        info!(
                            "[rewards] removed {removed} previously confirmed relay notifications"
                        );
                    }
                    confirmed_notifications_pruned = true;
                }
                Err(error) => error!("[rewards] confirmed notification cleanup failed: {error}"),
            }
        }
        tokio::select! {
            _ = retry.tick() => {},
            Some(_) = notifications.recv() => {},
            Some((mut job, result)) = done_rx.recv() => {
                let key = job.work.key();
                if matches!(job.work, MatchWork::Archive { .. }) { archives.remove(&key); }
                else if matches!(job.work, MatchWork::Notify { .. }) { notifications_in_flight.remove(&key); }
                else { verifiers.remove(job.work.match_id()); }
                match result {
                    Ok(true) => {
                        if matches!(job.work, MatchWork::Notify { .. }) {
                            if let Err(error) = state.db.remove_match_work(&key) {
                                error!("[rewards] applied notification {key} could not be removed: {error}");
                                job.retry_at = now() + 5;
                                job.last_error = Some(error.to_string());
                                if let Err(save_error) = state.db.save_match_work(&job) {
                                    error!("[rewards] notification {key} retry state could not be saved: {save_error}");
                                }
                            }
                        } else {
                            match state.db.remove_match_work(&key) {
                                Ok(()) => { let _ = state.match_results.send(job.work.match_id().to_string()); }
                                Err(error) => error!("[rewards] completed work {key} could not be removed: {error}"),
                            }
                        }
                    }
                    Ok(false) => {
                        job.state = MatchExitState::AwaitingResult;
                        job.retry_at = 0;
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
            if let Some(index) = jobs
                .iter()
                .position(|job| matches!(job.work, MatchWork::Progress { .. }) && !job.urgent)
            {
                let progress = jobs.remove(index);
                jobs.insert(0, progress);
            }
            closure_streak = 0;
        }
        for mut job in jobs {
            let key = job.work.key();
            let is_archive = matches!(job.work, MatchWork::Archive { .. });
            if is_archive {
                if archives.contains(&key) || archives.len() >= 1 {
                    continue;
                }
                archives.insert(key.clone());
            } else if matches!(job.work, MatchWork::Notify { .. }) {
                if notifications_in_flight.len() >= 32
                    || !notifications_in_flight.insert(key.clone())
                {
                    continue;
                }
            } else {
                if verifiers.len() >= parallelism || verifiers.contains(job.work.match_id()) {
                    continue;
                }
                verifiers.insert(job.work.match_id().to_string());
                if job.urgent {
                    closure_streak = closure_streak.saturating_add(1);
                }
                job.state = MatchExitState::Verifying;
                if let Err(error) = state.db.save_match_work(&job) {
                    error!("[rewards] work {key} cannot enter verification: {error}");
                    verifiers.remove(job.work.match_id());
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
                    "[rewards-timing] work={key} stage=queue_age elapsed_ms={}",
                    if job.queued_at_ms == 0 {
                        0
                    } else {
                        current_ms.saturating_sub(job.queued_at_ms)
                    }
                );
                info!("[rewards] work {key} starting");
                let attempt_started = Instant::now();
                let result = match &job.work {
                    MatchWork::Progress { .. } => verify_progress(&state, &job.work).await,
                    MatchWork::Archive { .. } => archive_progress_work(&state, &job.work).await,
                    MatchWork::Notify { command } => notify_result(&state, command).await,
                };
                let outcome = match &result {
                    Ok(true) => "complete",
                    Ok(false) => "pending",
                    Err(_) => "retry",
                };
                info!(
                    "[rewards-timing] work={key} stage=attempt elapsed_ms={} outcome={outcome}",
                    attempt_started.elapsed().as_millis()
                );
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
        assert!(permanent_progress_rejection(
            reqwest::StatusCode::BAD_REQUEST
        ));
        assert!(permanent_progress_rejection(
            reqwest::StatusCode::UNPROCESSABLE_ENTITY
        ));
        assert!(!permanent_progress_rejection(reqwest::StatusCode::CONFLICT));
        assert!(!permanent_progress_rejection(
            reqwest::StatusCode::UNAUTHORIZED
        ));
        assert!(!permanent_progress_rejection(
            reqwest::StatusCode::SERVICE_UNAVAILABLE
        ));
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
