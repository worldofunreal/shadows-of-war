//! Forward opaque turn batches to the durable server-side recorder.
use super::*;
use serde_json::Value;
use sow_data::profile::{ProgressClosure, RelayMatchProgress};
use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex as StdMutex, atomic::AtomicBool};

enum Command {
    Append(Turn),
    Flush {
        closures: Vec<ProgressClosure>,
        reply: oneshot::Sender<Result<(), String>>,
    },
    Finish {
        closures: Vec<ProgressClosure>,
        reply: oneshot::Sender<Result<(), String>>,
    },
}

pub(super) struct Forwarder {
    live: Arc<StdMutex<VecDeque<Turn>>>,
    tx: mpsc::Sender<Command>,
    failed: Arc<AtomicBool>,
}

impl Forwarder {
    pub fn new(match_id: u64) -> Arc<Self> {
        let (tx, rx) = mpsc::channel(PROGRESS_QUEUE_CAP);
        let failed = Arc::new(AtomicBool::new(false));
        tokio::spawn(run(match_id.to_string(), rx, failed.clone()));
        Arc::new(Self {
            live: Arc::new(StdMutex::new(VecDeque::with_capacity(LIVE_HISTORY_CAP))),
            tx,
            failed,
        })
    }

    pub fn append(&self, turn: Turn) -> Result<(), String> {
        {
            let mut live = self
                .live
                .lock()
                .map_err(|_| "live turn history lock failed")?;
            live.push_back(turn.clone());
            while live.len() > LIVE_HISTORY_CAP {
                live.pop_front();
            }
        }
        if self.failed.load(Ordering::Acquire) {
            return Err("server turn recording has failed".into());
        }
        match self.tx.try_send(Command::Append(turn)) {
            Ok(()) => Ok(()),
            Err(mpsc::error::TrySendError::Full(_)) => {
                self.failed.store(true, Ordering::Release);
                Err("server forwarding queue is full".into())
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                self.failed.store(true, Ordering::Release);
                Err("server forwarding task stopped".into())
            }
        }
    }

    pub fn snapshot(&self) -> Result<Vec<Turn>, String> {
        self.live
            .lock()
            .map(|turns| turns.iter().cloned().collect())
            .map_err(|_| "live turn history lock failed".into())
    }

    #[cfg(test)]
    pub(super) fn with_history(turns: Vec<Turn>) -> Arc<Self> {
        let (tx, _rx) = mpsc::channel(PROGRESS_QUEUE_CAP);
        Arc::new(Self {
            live: Arc::new(StdMutex::new(
                turns
                    .into_iter()
                    .rev()
                    .take(LIVE_HISTORY_CAP)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect(),
            )),
            tx,
            failed: Arc::new(AtomicBool::new(false)),
        })
    }

    #[cfg(test)]
    pub(super) fn with_manual_final_ack() -> (
        Arc<Self>,
        mpsc::Receiver<oneshot::Sender<Result<(), String>>>,
    ) {
        let (tx, mut rx) = mpsc::channel(PROGRESS_QUEUE_CAP);
        let (acks, responses) = mpsc::channel(2);
        tokio::spawn(async move {
            while let Some(command) = rx.recv().await {
                if let Command::Finish { reply, .. } = command {
                    if acks.send(reply).await.is_err() {
                        break;
                    }
                }
            }
        });
        (
            Arc::new(Self {
                live: Arc::new(StdMutex::new(VecDeque::new())),
                tx,
                failed: Arc::new(AtomicBool::new(false)),
            }),
            responses,
        )
    }

    pub async fn flush(&self, closures: Vec<ProgressClosure>) -> Result<(), String> {
        let (reply, response) = oneshot::channel();
        self.tx
            .send(Command::Flush { closures, reply })
            .await
            .map_err(|_| "server forwarding task stopped".to_string())?;
        response
            .await
            .map_err(|_| "server forwarding acknowledgment lost".to_string())?
    }

    pub async fn finish(&self, closures: Vec<ProgressClosure>) -> Result<(), String> {
        let (reply, response) = oneshot::channel();
        self.tx
            .send(Command::Finish { closures, reply })
            .await
            .map_err(|_| "server forwarding task stopped".to_string())?;
        response
            .await
            .map_err(|_| "server forwarding acknowledgment lost".to_string())?
    }
}

struct Stream {
    match_id: String,
    sequence: u64,
    first_turn: u64,
    turns: Vec<Turn>,
    bytes: usize,
    failed: bool,
    delivered_closures: HashSet<(String, u64)>,
}

impl Stream {
    fn new(match_id: String) -> Self {
        Self {
            match_id,
            sequence: 0,
            first_turn: 0,
            turns: Vec::new(),
            bytes: 8,
            failed: false,
            delivered_closures: HashSet::new(),
        }
    }

    fn append(&mut self, turn: Turn) -> Result<bool, String> {
        let expected_turn = self
            .first_turn
            .checked_add(self.turns.len() as u64)
            .ok_or("turn sequence overflow")?;
        if turn.turn_number != expected_turn {
            return Err(format!(
                "expected turn {expected_turn}, received {}",
                turn.turn_number
            ));
        }
        let turn_bytes =
            bincode::serialized_size(&turn).map_err(|error| error.to_string())? as usize;
        if turn_bytes + 8 > sow_data::MAX_MATCH_CHUNK_BYTES {
            return Err("one turn exceeds the server batch limit".into());
        }
        if self.bytes + turn_bytes > sow_data::MAX_MATCH_CHUNK_BYTES {
            return Err("turn batch exceeded the server limit before flush".into());
        }
        self.bytes += turn_bytes;
        self.turns.push(turn);
        Ok(self.bytes >= sow_data::MAX_MATCH_CHUNK_BYTES / 2)
    }

    async fn seal(
        &mut self,
        closures: &[ProgressClosure],
        finished: bool,
        failed: bool,
    ) -> Result<bool, String> {
        if failed || self.failed {
            self.reject(closures, finished).await?;
            return Ok(true);
        }
        let end_turn = self.first_turn.saturating_add(self.turns.len() as u64);
        let deliver_closures = closures
            .iter()
            .filter(|closure| {
                closure.turn_number < end_turn
                    && !self
                        .delivered_closures
                        .contains(&(closure.account_id.clone(), closure.turn_number))
            })
            .cloned()
            .collect::<Vec<_>>();
        let closures_pending = closures.iter().any(|closure| {
            closure.turn_number >= end_turn
                || !self
                    .delivered_closures
                    .contains(&(closure.account_id.clone(), closure.turn_number))
                    && !deliver_closures.iter().any(|delivered| {
                        delivered.account_id == closure.account_id
                            && delivered.turn_number == closure.turn_number
                    })
        });
        if finished && closures_pending {
            return Ok(false);
        }
        if self.turns.is_empty() && deliver_closures.is_empty() && !finished {
            return Ok(!closures_pending);
        }
        let replay_data = bincode::serialize(&self.turns).map_err(|error| error.to_string())?;
        if replay_data.len() > sow_data::MAX_MATCH_CHUNK_BYTES {
            return Err("turn batch exceeded the server limit after encoding".into());
        }
        let batch = RelayMatchProgress {
            match_id: self.match_id.clone(),
            sequence: self.sequence,
            first_turn: self.first_turn,
            replay_data,
            participant_closures: deliver_closures.clone(),
            finished,
            recording_failed: false,
        };
        let ack = deliver_until_accepted(&batch).await?;
        if ack.get("match_id").and_then(Value::as_str) != Some(self.match_id.as_str())
            || ack.get("sequence").and_then(Value::as_u64) != Some(self.sequence)
            || ack.get("sha256").and_then(Value::as_str).is_none()
        {
            self.failed = true;
            self.reject(closures, finished).await?;
            return Ok(true);
        }
        for closure in deliver_closures {
            self.delivered_closures
                .insert((closure.account_id, closure.turn_number));
        }
        self.sequence += 1;
        self.first_turn = end_turn;
        self.turns.clear();
        self.bytes = 8;
        Ok(!closures_pending)
    }

    async fn reject(&mut self, closures: &[ProgressClosure], finished: bool) -> Result<(), String> {
        self.failed = true;
        let batch = RelayMatchProgress {
            match_id: self.match_id.clone(),
            sequence: self.sequence,
            first_turn: self.first_turn,
            replay_data: Vec::new(),
            participant_closures: closures.to_vec(),
            finished,
            recording_failed: true,
        };
        let ack = deliver_until_accepted(&batch).await?;
        if ack.get("match_id").and_then(Value::as_str) != Some(self.match_id.as_str())
            || ack.get("sequence").and_then(Value::as_u64) != Some(self.sequence)
            || ack.get("recording_failed").and_then(Value::as_bool) != Some(true)
        {
            return Err("server did not acknowledge failed recording".into());
        }
        self.turns.clear();
        self.bytes = 8;
        Ok(())
    }
}

async fn deliver_until_accepted(batch: &RelayMatchProgress) -> Result<Value, String> {
    let db_url = configured_db_url()?;
    let secret = std::env::var("SOW_RELAY_SETTLEMENT_SECRET").map_err(|error| error.to_string())?;
    let url = format!("{}/internal/match-turns", db_url.trim_end_matches('/'));
    let body = bincode::serialize(batch).map_err(|error| error.to_string())?;
    let client = db_client(&db_url)?;
    let mut attempt = 0u64;
    loop {
        attempt += 1;
        match client
            .post(&url)
            .bearer_auth(&secret)
            .header("Content-Type", "application/octet-stream")
            .body(body.clone())
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => {
                return response.json().await.map_err(|error| error.to_string());
            }
            Ok(response)
                if response.status().is_client_error() && response.status().as_u16() != 429 =>
            {
                let status = response.status();
                let reason = response.text().await.unwrap_or_default();
                return Err(format!(
                    "server rejected match {} sequence {}: HTTP {status}: {reason}",
                    batch.match_id, batch.sequence
                ));
            }
            Ok(response) => {
                if attempt == 1 || attempt % 10 == 0 {
                    warn!(
                        "[rewards] server turn delivery pending match={} sequence={} http={} attempt={attempt}",
                        batch.match_id,
                        batch.sequence,
                        response.status()
                    );
                }
            }
            Err(error) => {
                if attempt == 1 || attempt % 10 == 0 {
                    warn!(
                        "[rewards] server turn delivery pending match={} sequence={} error={error} attempt={attempt}",
                        batch.match_id, batch.sequence
                    );
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

async fn run(match_id: String, mut rx: mpsc::Receiver<Command>, failed: Arc<AtomicBool>) {
    let mut stream = Stream::new(match_id);
    let mut closures = Vec::new();
    let mut flush = interval(Duration::from_secs(
        sow_data::MATCH_PROGRESS_INTERVAL_SECONDS,
    ));
    flush.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            command = rx.recv() => {
                let Some(command) = command else { break; };
                match command {
                    Command::Append(turn) => {
                        if failed.load(Ordering::Acquire) { stream.failed = true; }
                        if stream.failed { continue; }
                        match stream.append(turn) {
                            Ok(true) => if let Err(error) = stream.seal(&closures, false, false).await {
                                stream.failed = true;
                                error!("[rewards] server turn delivery failed match={}: {error}", stream.match_id);
                                if let Err(failure) = stream.reject(&closures, false).await {
                                    error!("[rewards] failed recording remains unacknowledged match={}: {failure}", stream.match_id);
                                }
                            },
                            Ok(false) => {},
                            Err(error) => {
                                stream.failed = true;
                                error!("[rewards] server turn batch failed match={}: {error}", stream.match_id);
                                if let Err(failure) = stream.reject(&closures, false).await {
                                    error!("[rewards] failed recording remains unacknowledged match={}: {failure}", stream.match_id);
                                }
                            }
                        }
                    }
                    Command::Flush { closures: latest, reply } => {
                        closures = latest;
                        let result = stream.seal(&closures, false, failed.load(Ordering::Acquire)).await
                            .and_then(|committed| if committed { Ok(()) } else { Err("exit closure awaits its server turn".into()) });
                        let _ = reply.send(result);
                    }
                    Command::Finish { closures: latest, reply } => {
                        closures = latest;
                        let result = stream.seal(&closures, true, failed.load(Ordering::Acquire)).await
                            .and_then(|committed| if committed { Ok(()) } else { Err("final exit closure awaits its server turn".into()) });
                        let completed = result.is_ok();
                        let _ = reply.send(result);
                        if completed { break; }
                    }
                }
            }
            _ = flush.tick() => {
                if let Err(error) = stream.seal(&closures, false, failed.load(Ordering::Acquire)).await {
                    stream.failed = true;
                    error!("[rewards] periodic server turn delivery failed match={}: {error}", stream.match_id);
                    if let Err(failure) = stream.reject(&closures, false).await {
                        error!("[rewards] failed recording remains unacknowledged match={}: {failure}", stream.match_id);
                    }
                }
            }
        }
    }
}
