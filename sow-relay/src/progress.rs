//! Bounded delivery of the existing durable journal, outside the game tick.
use super::*;
use sow_data::profile::{MatchProgress, ProgressClosure};
use tokio::io::AsyncReadExt;

static DELIVERY: tokio::sync::Notify = tokio::sync::Notify::const_new();

pub(super) struct Stream {
    id: String,
    sequence: u64,
    first_turn: u64,
    previous: String,
    turns: Vec<Turn>,
    pub bytes: usize,
}

impl Stream {
    pub fn new(id:String) -> Self {
        Self { id, sequence:0, first_turn:0, previous:String::new(), turns:Vec::new(), bytes:8 }
    }
    pub fn append(&mut self, turn:Turn, bytes:usize) {
        self.bytes += bytes;
        self.turns.push(turn);
    }
    pub async fn seal(&mut self, file:&mut tokio::fs::File, journal:&Path, metadata:&ReplaySpoolMetadata, finished:bool) -> Result<(),String> {
        if self.turns.is_empty() && !finished { return Ok(()); }
        if self.bytes > sow_data::MAX_MATCH_CHUNK_BYTES { return Err("one turn exceeds the journal delivery limit".into()); }
        file.flush().await.map_err(|e|e.to_string())?;
        file.sync_all().await.map_err(|e|e.to_string())?;
        let count = self.turns.len() as u64;
        let mut batch = MatchProgress { match_id:self.id.clone(),sequence:self.sequence,first_turn:self.first_turn,
            previous_sha256:self.previous.clone(),sha256:String::new(),replay_data:bincode::serialize(&self.turns).map_err(|e|e.to_string())?,
            participant_closures:metadata.participant_closures.iter().filter(|c| c.turn_number < self.first_turn+count).map(|c| ProgressClosure {
                account_id:c.account_id.clone(), turn_number:c.turn_number,
                reason:serde_json::to_value(c.reason).unwrap().as_str().unwrap().to_string() }).collect(), finished };
        batch.sha256 = hex::encode(Sha256::digest(batch.signing_bytes().map_err(|e|e.to_string())?));
        let path = journal.with_file_name(format!("{}.progress.{:010}.json",self.id,self.sequence));
        let _claim = claim_result(path.clone()).await?;
        let temporary = path.with_extension("json.tmp");
        let mut output = tokio::fs::File::create(&temporary).await.map_err(|e|e.to_string())?;
        output.write_all(&serde_json::to_vec(&batch).map_err(|e|e.to_string())?).await.map_err(|e|e.to_string())?;
        output.sync_all().await.map_err(|e|e.to_string())?;
        drop(output);
        tokio::fs::rename(temporary,&path).await.map_err(|e|e.to_string())?;
        std::fs::File::open(path.parent().unwrap()).and_then(|d|d.sync_all()).map_err(|e|e.to_string())?;
        self.previous = batch.sha256;
        self.sequence += 1;
        self.first_turn += count;
        self.turns.clear();
        self.bytes = 8;
        DELIVERY.notify_one();
        Ok(())
    }
}

async fn files() -> Result<Vec<PathBuf>,String> {
    let mut entries = match tokio::fs::read_dir(replay_spool_dir()).await {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    let mut paths = Vec::new();
    while let Some(entry) = entries.next_entry().await.map_err(|e|e.to_string())? {
        let path = entry.path();
        if path.file_name().is_some_and(|n| n.to_string_lossy().contains(".progress."))
            && matches!(path.extension().and_then(|e|e.to_str()),Some("json" | "acked")) { paths.push(path); }
    }
    paths.sort();
    Ok(paths)
}

async fn deliver(path:&Path) -> Result<(),String> {
    let _claim = claim_result(path.to_path_buf()).await?;
    let bytes = match tokio::fs::read(path).await {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.to_string()),
    };
    let batch: MatchProgress = serde_json::from_slice(&bytes).map_err(|e|e.to_string())?;
    let db = configured_db_url()?;
    let secret = std::env::var("SOW_RELAY_SETTLEMENT_SECRET").map_err(|e|e.to_string())?;
    let started = Instant::now();
    let response = db_client(&db)?.post(format!("{}/internal/match-progress",db.trim_end_matches('/')))
        .bearer_auth(secret).header("Content-Type","application/json").body(bytes).send().await.map_err(|e|e.to_string())?;
    if !response.status().is_success() { return Err(format!("journal delivery HTTP {}: {}",response.status(),response.text().await.map_err(|e|e.to_string())?)); }
    let ack: serde_json::Value = response.json().await.map_err(|e|e.to_string())?;
    if ack.get("match_id").and_then(|v|v.as_str()) != Some(batch.match_id.as_str())
        || ack.get("sequence").and_then(|v|v.as_u64()) != Some(batch.sequence)
        || ack.get("sha256").and_then(|v|v.as_str()) != Some(batch.sha256.as_str()) {
        return Err("journal acknowledgment identity mismatch".into());
    }
    tokio::fs::rename(path,path.with_extension("acked")).await.map_err(|e|e.to_string())?;
    std::fs::File::open(path.parent().unwrap()).and_then(|d|d.sync_all()).map_err(|e|e.to_string())?;
    info!("[exit-timing] journal delivered match={} sequence={} first_turn={} delivery_ms={}",batch.match_id,batch.sequence,batch.first_turn,started.elapsed().as_millis());
    Ok(())
}

async fn deliver_match(id:&str) -> Result<(),String> {
    let prefix = format!("{id}.progress.");
    for path in files().await? {
        if path.file_name().unwrap().to_string_lossy().starts_with(&prefix)
            && path.extension().and_then(|e|e.to_str()) == Some("json") { deliver(&path).await?; }
    }
    Ok(())
}

pub(super) async fn finish_delivery(replay:&Path) -> Result<(),String> {
    let id = replay.file_stem().unwrap().to_string_lossy().to_string();
    deliver_match(&id).await?;
    let prefix = format!("{id}.progress.");
    for path in files().await? {
        if path.file_name().unwrap().to_string_lossy().starts_with(&prefix) {
            tokio::fs::remove_file(path).await.map_err(|e|e.to_string())?;
        }
    }
    cleanup_result_spool(replay,&replay.with_extension("json")).await
}

pub(super) async fn delivery_worker() {
    let mut timer = interval(Duration::from_secs(1));
    let mut running = HashSet::new();
    let mut tasks = tokio::task::JoinSet::new();
    loop {
        tokio::select! {
            _ = timer.tick() => {},
            _ = DELIVERY.notified() => {},
            Some(result) = tasks.join_next() => match result {
                Ok((id,result)) => { running.remove(&id); if let Err(error) = result { warn!("journal delivery pending match={id}: {error}"); } },
                Err(error) => error!("journal delivery task failed: {error}"),
            },
        }
        let paths = match files().await { Ok(paths)=>paths,Err(error)=>{ warn!("journal delivery scan: {error}"); continue; } };
        for path in paths {
            if path.extension().and_then(|e|e.to_str()) != Some("json") { continue; }
            let id = path.file_name().unwrap().to_string_lossy().split('.').next().unwrap().to_string();
            if running.len() >= 8 || !running.insert(id.clone()) { continue; }
            tasks.spawn(async move { let result = deliver_match(&id).await; (id,result) });
        }
    }
}

pub(super) async fn recover(journal:&Path, metadata:&mut ReplaySpoolMetadata) -> Result<bool,String> {
    let id = journal.file_stem().unwrap().to_string_lossy().to_string();
    let prefix = format!("{id}.progress.");
    let latest = files().await?.into_iter().filter(|p|p.file_name().unwrap().to_string_lossy().starts_with(&prefix)).last();
    let mut stream = Stream::new(id.clone());
    let mut already_finished = false;
    if let Some(path) = latest {
        let saved: MatchProgress = serde_json::from_slice(&tokio::fs::read(path).await.map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
        stream.sequence = saved.sequence + 1;
        stream.first_turn = saved.first_turn + u64::from_le_bytes(saved.replay_data[..8].try_into().unwrap());
        stream.previous = saved.sha256;
        already_finished = saved.finished;
    }
    let mut input = tokio::fs::File::open(journal).await.map_err(|e|e.to_string())?;
    let mut output = tokio::fs::OpenOptions::new().append(true).open(journal).await.map_err(|e|e.to_string())?;
    let mut next = 0;
    let mut valid_bytes = 0u64;
    let mut resigned = HashSet::new();
    if !already_finished {
        loop {
            let mut length = [0;4];
            match input.read_exact(&mut length).await {
                Ok(_) => {},
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e.to_string()),
            }
            let length = u32::from_le_bytes(length) as usize;
            if length > sow_data::MAX_MATCH_CHUNK_BYTES { return Err("recovery journal turn exceeds limit".into()); }
            let mut bytes = vec![0;length];
            match input.read_exact(&mut bytes).await {
                Ok(_) => {},
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e.to_string()),
            }
            let turn: Turn = bincode::deserialize(&bytes).map_err(|e|e.to_string())?;
            if turn.turn_number != next { return Err("recovery journal sequence mismatch".into()); }
            next += 1;
            valid_bytes += 4 + length as u64;
            for intent in &turn.intents { if matches!(intent.intent,GameplayIntent::Resign) { resigned.insert(intent.player_id); } }
            if turn.turn_number >= stream.first_turn {
                if stream.bytes + bytes.len() > sow_data::MAX_MATCH_CHUNK_BYTES { stream.seal(&mut output,journal,metadata,false).await?; }
                stream.append(turn,bytes.len());
            }
        }
        output.set_len(valid_bytes).await.map_err(|e|e.to_string())?;
        let roster: RegisterBody = serde_json::from_str(&metadata.lobby_json).map_err(|e|e.to_string())?;
        let mut intents = Vec::new();
        for player in roster.players.iter().filter(|p|!p.is_internal) {
            if resigned.contains(&player.player_id) || player.database_account_id.as_ref().is_some_and(|a|metadata.participant_closures.iter().any(|c| &c.account_id == a)) { continue; }
            intents.push(StampedIntent { player_id:player.player_id,intent:GameplayIntent::Resign });
            if let Some(account) = &player.database_account_id {
                metadata.participant_closures.push(ParticipantClosure { account_id:account.clone(),turn_number:next,reason:ExitReason::MatchEnded });
            }
        }
        if !intents.is_empty() {
            let turn = Turn { turn_number:next,intents };
            let bytes = bincode::serialize(&turn).map_err(|e|e.to_string())?;
            output.write_all(&(bytes.len() as u32).to_le_bytes()).await.map_err(|e|e.to_string())?;
            output.write_all(&bytes).await.map_err(|e|e.to_string())?;
            stream.append(turn,bytes.len());
        }
        stream.seal(&mut output,journal,metadata,true).await?;
    }
    metadata.phase = ReplayPhase::StreamFinalized;
    tokio::fs::rename(journal,journal.with_extension("replay")).await.map_err(|e|e.to_string())?;
    persist_replay_metadata(&journal.with_extension("json"),metadata).await?;
    Ok(true)
}
