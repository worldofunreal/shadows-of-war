//! One durable owner for return commands, personal results and whole replays.
use super::*;
use sha2::{Digest, Sha256};
use sow_data::profile::{
    MatchExitState, MatchWork, MatchWorkRecord, ReplayVerificationStatus, RewardSettlementStatus,
};
use std::path::PathBuf;

type WorkResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

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
    participant_path(match_id, "").with_file_name(format!("{match_id}.progress.{sequence:010}.json"))
}

pub(super) async fn handle_progress(State(state): State<Arc<AppState>>, headers: HeaderMap,
    Json(batch): Json<sow_data::profile::MatchProgress>) -> axum::response::Response {
    if !verify_internal_auth(&headers, &state.relay_settlement_secret) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let result = async {
        if batch.match_id.parse::<u64>().is_err() || batch.replay_data.len() > sow_data::MAX_MATCH_CHUNK_BYTES
            || batch.replay_data.len() < 8 || hex::encode(Sha256::digest(batch.signing_bytes()?)) != batch.sha256 {
            return Err("invalid journal delivery".into());
        }
        let _guard = state.match_artifact_lock.lock().await;
        let start = state.db.match_start_record(&batch.match_id)?.ok_or("trusted match registration unavailable")?;
        let tick_rate = start.metadata.pointer("/config/tick_rate_ms").and_then(serde_json::Value::as_f64)
            .ok_or("trusted clock missing")? as f32;
        let count = u64::from_le_bytes(batch.replay_data[..8].try_into().unwrap());
        if !tick_rate.is_finite() || tick_rate <= 0.0 || count > sow_data::max_match_turns(tick_rate)
            || batch.first_turn.saturating_add(count) > sow_data::max_match_turns(tick_rate) {
            return Err("invalid journal duration".into());
        }
        let path = progress_path(&batch.match_id, batch.sequence);
        let existing = tokio::fs::read(&path).await;
        match existing {
            Ok(bytes) => {
                let saved: sow_data::profile::MatchProgress = serde_json::from_slice(&bytes)?;
                if saved != batch { return Err("journal delivery conflicts with saved state".into()); }
                if !tokio::fs::try_exists(path.with_extension("done")).await? {
                    let job = state.db.queue_match_work(progress_work(&batch))?;
                    wake(&state,job.work.key());
                }
                // The completion marker distinguishes an ACK retry from a
                // crash between the artifact rename and the queue commit.
                return Ok(serde_json::json!({"match_id":batch.match_id,"sequence":batch.sequence,"sha256":batch.sha256}));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
            Err(e) => return Err(e.into()),
        }
        if batch.sequence == 0 {
            if batch.first_turn != 0 || !batch.previous_sha256.is_empty() { return Err("journal must start at turn zero".into()); }
        } else {
            let bytes = tokio::fs::read(progress_path(&batch.match_id,batch.sequence-1)).await?;
            let previous: sow_data::profile::MatchProgress = serde_json::from_slice(&bytes)?;
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
        let temporary = path.with_extension("json.tmp");
        let mut file = tokio::fs::File::create(&temporary).await?;
        file.write_all(&serde_json::to_vec(&batch)?).await?;
        file.sync_all().await?;
        drop(file);
        tokio::fs::rename(temporary,&path).await?;
        std::fs::File::open(parent)?.sync_all()?;
        let job = state.db.queue_match_work(progress_work(&batch))?;
        wake(&state, job.work.key());
        Ok(serde_json::json!({"match_id":batch.match_id,"sequence":batch.sequence,"sha256":batch.sha256}))
    }.await;
    match result {
        Ok::<_,Box<dyn std::error::Error+Send+Sync>>(ack) => (StatusCode::ACCEPTED,Json(ack)).into_response(),
        Err(e) => {
            warn!("[rewards] journal intake pending match={}: {e}", batch.match_id);
            (StatusCode::SERVICE_UNAVAILABLE,Json(ErrorResponse { error:e.to_string() })).into_response()
        }
    }
}

fn progress_work(batch: &sow_data::profile::MatchProgress) -> MatchWork {
    MatchWork::Progress { match_id:batch.match_id.clone(), sequence:batch.sequence, sha256:batch.sha256.clone(),
        closing:batch.finished || !batch.participant_closures.is_empty() }
}

async fn verify_progress(state: &AppState, work: &MatchWork) -> WorkResult<bool> {
    let MatchWork::Progress { match_id, sequence, sha256, .. } = work else { return Err("invalid journal work".into()); };
    let bytes = tokio::fs::read(progress_path(match_id,*sequence)).await?;
    let batch: sow_data::profile::MatchProgress = serde_json::from_slice(&bytes)?;
    if &batch.sha256 != sha256 || hex::encode(Sha256::digest(batch.signing_bytes()?)) != *sha256 {
        return Err("saved journal checksum mismatch".into());
    }
    let base = std::env::var("SOW_SERVER_HTTP_URL").unwrap_or_else(|_| "http://127.0.0.1:25566".into());
    let response = state.replay_verification_client.post(format!("{}/internal/match/progress",base.trim_end_matches('/')))
        .bearer_auth(&state.secret_token).header("Content-Type","application/json").body(bytes).send().await?;
    if response.status() == reqwest::StatusCode::CONFLICT {
        // A restart can restore an earlier checkpoint. Requeue durable chunks;
        // checked turn hashes skip the prefix still held by the verifier.
        for prior in 0..*sequence {
            let bytes = tokio::fs::read(progress_path(match_id,prior)).await?;
            let saved: sow_data::profile::MatchProgress = serde_json::from_slice(&bytes)?;
            state.db.queue_match_work(progress_work(&saved))?;
        }
        return Err("verifier is restoring saved journal history".into());
    }
    if !response.status().is_success() { return Err(format!("journal verifier HTTP {}: {}", response.status(),response.text().await?).into()); }
    let checked = response.json::<sow_data::profile::VerifiedProgress>().await?;
    if checked.match_id != *match_id || checked.sequence != *sequence || checked.sha256 != *sha256 {
        return Err("verifier returned another journal delivery".into());
    }
    let mut closures = Vec::new();
    for outcome in &checked.participants {
        if outcome.result.match_id != *match_id { return Err("verifier returned another match".into()); }
        let turn = outcome.result.personal_closure_turn.ok_or("verified closure turn missing")?;
        state.db.note_match_participant_closure(match_id,&outcome.account_id,turn).await?;
        state.db.apply_verified_match_participant_result(&outcome.result,&outcome.account_id,turn).await?;
        closures.push(sow_data::profile::ProgressClosure { account_id:outcome.account_id.clone(),turn_number:turn,reason:outcome.reason.clone() });
    }
    if let Some(result) = &checked.result {
        state.db.finalize_match_with_lobby(match_id,None).await?;
        state.db.apply_verified_match_result(result).await?;
        state.db.finish_replay_verification(match_id,ReplayVerificationStatus::Verified)?;
    }
    let paid = progress_path(match_id,*sequence).with_extension("paid");
    let mut file = tokio::fs::File::create(&paid).await?;
    file.write_all(checked.verified_turns.to_string().as_bytes()).await?;
    file.sync_all().await?;
    std::fs::File::open(paid.parent().unwrap())?.sync_all()?;
    let _ = state.match_results.send(match_id.clone());
    if checked.terminal_turn.is_some() || !closures.is_empty() {
        let response = state.replay_verification_client.post(format!("{}/internal/match/result",base.trim_end_matches('/')))
            .bearer_auth(&state.secret_token).json(&sow_data::profile::MatchResultControl {
                match_id:match_id.clone(),terminal_turn:checked.terminal_turn,participant_closures:closures }).send().await?;
        if !response.status().is_success() { return Err(format!("verified result delivery HTTP {}",response.status()).into()); }
    }
    if checked.complete && batch.finished {
        // Packing the public recording follows payment and uses bounded chunks.
        archive_progress(state,&batch).await?;
    }
    if checked.complete {
        let path = progress_path(match_id,*sequence).with_extension("done");
        let mut file = tokio::fs::File::create(&path).await?;
        file.write_all(batch.sha256.as_bytes()).await?;
        file.sync_all().await?;
        std::fs::File::open(path.parent().unwrap())?.sync_all()?;
    }
    Ok(checked.complete)
}

async fn archive_progress(state: &AppState, last: &sow_data::profile::MatchProgress) -> WorkResult<()> {
    use tokio::io::AsyncSeekExt;
    let path = participant_path(&last.match_id, "").with_file_name(format!("{}.replay",last.match_id));
    let temporary = path.with_extension("replay.tmp");
    let mut file = tokio::fs::File::create(&temporary).await?;
    file.write_all(&0u64.to_le_bytes()).await?;
    let mut turns = 0u64;
    for sequence in 0..=last.sequence {
        let bytes = tokio::fs::read(progress_path(&last.match_id,sequence)).await?;
        let batch: sow_data::profile::MatchProgress = serde_json::from_slice(&bytes)?;
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
        for sequence in state.db.pending_match_progress(match_id)? {
            if !progress_path(match_id,sequence).with_extension("paid").try_exists()? {
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
        let bytes = tokio::fs::read(progress_path(match_id,sequence)).await?;
        let batch: sow_data::profile::MatchProgress = serde_json::from_slice(&bytes)?;
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
    let mut verifier: Option<String> = None;
    let mut closure_streak = 0u8;
    let mut retry = tokio::time::interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            _ = retry.tick() => {},
            Some(_) = notifications.recv() => {},
            Some((mut job, result)) = done_rx.recv() => {
                let key = job.work.key();
                if matches!(job.work, MatchWork::RecoveryExit { .. }) { exits.remove(&key); } else { verifier = None; }
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
            if is_exit {
                if exits.contains(&key) || exits.len() >= 8 {
                    continue;
                }
                exits.insert(key.clone());
            } else {
                if verifier.is_some() {
                    continue;
                }
                verifier = Some(key.clone());
                if !matches!(job.work, MatchWork::Progress { .. }) || job.urgent { closure_streak = closure_streak.saturating_add(1); }
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
