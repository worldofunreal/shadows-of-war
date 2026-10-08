//! Product analytics event schema, closed taxonomy, and durable JSONL sink.
//!
//! Consented first-party events are validated against a closed taxonomy and
//! appended to one daily file. Account IDs are replaced by channel-scoped
//! server hashes before reaching this sink.

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: u8 = 2;
pub const CONSENT_POLICY_VERSION: &str = "2026-10-06";
pub const RETENTION_DAYS: u64 = 90;
pub const MAX_PROPS_BYTES: usize = 2048;
const MAX_STRING_FIELD: usize = 64;

/// Closed taxonomy. Unknown names are rejected at ingest — adding an event
/// means adding it here first.
pub const EVENT_NAMES: &[&str] = &[
    "landing_visit",
    "shell_loaded",
    "play_now_click",
    "boot_start",
    "boot_route_decision",
    "load_stage",
    "menu_quick_match",
    "menu_join_attempt",
    "menu_password_join_attempt",
    "menu_code_join_attempt",
    "menu_custom_create",
    "menu_single_player_start",
    "menu_campaign_start",
    "menu_campaign_open",
    "menu_lobby_browser_open",
    "menu_custom_create_open",
    "menu_leader_confirm",
    "matchmaking_joined",
    "lobby_joined",
    "lobby_join_failed",
    "match_exit",
    "match_loading_start",
    "match_started_client",
    "match_ended_client",
    "tutorial_start",
    "tutorial_step",
    "tutorial_objective_complete",
    "tutorial_dialog_choice",
    "tutorial_exit_early",
    "campaign_episode_complete",
];

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct AnalyticsEvent {
    #[serde(default)]
    pub v: u8,
    pub event_id: String,
    pub consented: bool,
    pub consent_version: String,
    pub name: String,
    pub ts_ms: u64,
    #[serde(default)]
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_id: Option<String>,
    #[serde(default)]
    pub portal: Option<String>,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_class: Option<String>,
    #[serde(default)]
    pub build: Option<String>,
    #[serde(default)]
    pub locale: Option<String>,
    #[serde(default)]
    pub props: Option<serde_json::Value>,
}

impl AnalyticsEvent {
    /// Pure validation; `now_ms` injected so tests stay deterministic.
    pub fn validate(&self, now_ms: u64) -> Result<(), &'static str> {
        if self.v != SCHEMA_VERSION {
            return Err("unsupported schema version");
        }
        if !is_uuid(&self.event_id) {
            return Err("invalid event_id");
        }
        if !self.consented || self.consent_version != CONSENT_POLICY_VERSION {
            return Err("analytics consent is missing or outdated");
        }
        if self.subject_id.is_some() {
            return Err("subject_id is server-owned");
        }
        if self.portal.as_deref() != Some("site") || self.platform.as_deref() != Some("web") {
            return Err("unsupported analytics channel");
        }
        if !EVENT_NAMES.contains(&self.name.as_str()) {
            return Err("unknown event name");
        }
        let minute_ms: u64 = 60_000;
        if self.ts_ms < 1_600_000_000_000 || self.ts_ms > now_ms.saturating_add(5 * minute_ms) {
            return Err("timestamp out of range");
        }
        if self.session_id.is_empty()
            || self.session_id.len() > MAX_STRING_FIELD
            || !is_simple_string(&self.session_id)
        {
            return Err("invalid session_id");
        }
        if let Some(id) = &self.account_id
            && (id.len() != 32 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()))
        {
            return Err("invalid account_id");
        }
        if self.build.as_deref().is_some_and(|value| {
            !value.is_empty()
                && !value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
        }) {
            return Err("invalid build");
        }
        if let Some(id) = &self.subject_id
            && (id.len() != 64 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()))
        {
            return Err("invalid subject_id");
        }
        for field in [&self.portal, &self.platform, &self.build, &self.locale] {
            if let Some(value) = field
                && (!value.is_empty() && (value.len() > 32 || !is_simple_string(value)))
            {
                return Err("invalid string field");
            }
        }
        if self.device_class.as_deref().is_some_and(|value| {
            !matches!(value, "mobile" | "tablet" | "desktop" | "unknown")
        }) {
            return Err("invalid device_class");
        }
        if let Some(props) = &self.props {
            if serde_json::to_vec(props).map_or(true, |bytes| bytes.len() > MAX_PROPS_BYTES) {
                return Err("props too large");
            }
            if !valid_props(&self.name, props) {
                return Err("unsupported event properties");
            }
        }
        Ok(())
    }
}

fn is_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        })
}

fn valid_props(name: &str, props: &serde_json::Value) -> bool {
    let Some(props) = props.as_object() else {
        return false;
    };
    if name == "tutorial_step" {
        if props.len() == 1 {
            return props
                .get("step_index")
                .and_then(serde_json::Value::as_u64)
                .is_some_and(|index| index < 512);
        }
        return props.len() == 4
            && props
                .get("episode_id")
                .and_then(serde_json::Value::as_str)
                .is_some_and(is_episode_id)
            && props
                .get("step_id")
                .and_then(serde_json::Value::as_str)
                .is_some_and(is_step_id)
            && props
                .get("step_index")
                .and_then(serde_json::Value::as_u64)
                .is_some_and(|index| index < 512)
            && props
                .get("action")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|action| matches!(action, "start" | "complete"));
    }
    if name == "tutorial_exit_early" {
        if props.len() == 1 {
            return props
                .get("episode_id")
                .and_then(serde_json::Value::as_str)
                .is_some_and(is_episode_id);
        }
        return props.len() == 4
            && props
                .get("episode_id")
                .and_then(serde_json::Value::as_str)
                .is_some_and(is_episode_id)
            && props
                .get("step_id")
                .and_then(serde_json::Value::as_str)
                .is_some_and(is_step_id)
            && props
                .get("step_index")
                .and_then(serde_json::Value::as_u64)
                .is_some_and(|index| index < 512)
            && props
                .get("action")
                .and_then(serde_json::Value::as_str)
                == Some("fail");
    }
    if matches!(name, "tutorial_start" | "campaign_episode_complete") {
        return props.len() == 1
            && props
                .get("episode_id")
                .and_then(serde_json::Value::as_str)
                .is_some_and(is_episode_id);
    }
    if props.len() != 1 { return false; }
    let (key, value) = props.iter().next().expect("one property checked");
    match (name, key.as_str(), value.as_str()) {
        ("load_stage", "stage", Some(value)) => matches!(
            value,
            "relay_connect_start"
                | "relay_connect_complete"
                | "engine_init_complete"
                | "gpu_upload_complete"
                | "snapshot_available"
                | "ready_sent"
        ),
        ("boot_route_decision", "route", Some("menu" | "intro")) => true,
        _ => false,
    }
}

fn is_episode_id(value: &str) -> bool {
    value.len() <= 64
        && value
            .bytes()
            .enumerate()
            .all(|(index, byte)| byte.is_ascii_lowercase() || byte.is_ascii_digit() && index > 0 || byte == b'_' && index > 0)
        && value.as_bytes().first().is_some_and(|byte| byte.is_ascii_lowercase())
}

fn is_step_id(value: &str) -> bool {
    value.len() <= 96
        && value
            .bytes()
            .enumerate()
            .all(|(index, byte)| byte.is_ascii_lowercase() || byte.is_ascii_digit() && index > 0 || matches!(byte, b'_' | b'-') && index > 0)
        && value.as_bytes().first().is_some_and(|byte| byte.is_ascii_lowercase())
        && !matches!(value, "constructor" | "prototype" | "__proto__")
}

fn is_simple_string(value: &str) -> bool {
    !value.chars().any(char::is_control)
}

pub fn utc_date_string() -> String {
    let dt = crate::time_util::now_utc();
    format!("{:04}-{:02}-{:02}", dt.year, dt.month, dt.day)
}

pub fn utc_day_number() -> i64 {
    let dt = crate::time_util::now_utc();
    days_from_civil(dt.year, dt.month, dt.day)
}

/// Shift an ISO UTC date by a number of calendar days.
pub fn shift_date(date: &str, offset_days: i64) -> Option<String> {
    let mut parts = date.split('-');
    let year = parts.next()?.parse::<i64>().ok()?;
    let month = parts.next()?.parse::<u32>().ok()?;
    let day = parts.next()?.parse::<u32>().ok()?;
    if parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let days = days_from_civil(year, month, day).checked_add(offset_days)?;
    let (year, month, day) = civil_from_days(days);
    Some(format!("{year:04}-{month:02}-{day:02}"))
}

/// Appends validated event lines to one file per UTC day. Rotation happens on
/// write when the UTC date changes; pruning removes files past retention.
pub struct EventSink {
    dir: PathBuf,
    day: String,
    file: Option<std::fs::File>,
}

impl EventSink {
    pub fn new(dir: impl Into<PathBuf>) -> std::io::Result<Self> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)?;
        let sink = Self {
            dir,
            day: utc_date_string(),
            file: None,
        };
        sink.prune_old()?;
        Ok(sink)
    }

    pub fn append_line(&mut self, line: &str) -> std::io::Result<()> {
        self.append_batch(&[line.to_string()])
    }

    /// Append an accepted batch with one flush and durability sync.
    pub fn append_batch(&mut self, lines: &[String]) -> std::io::Result<()> {
        if lines.is_empty() {
            return Ok(());
        }
        let today = utc_date_string();
        if today != self.day || self.file.is_none() {
            self.rotate(today)?;
        }
        let file = self.file.as_mut().expect("rotated file must be open");
        for line in lines {
            file.write_all(line.as_bytes())?;
            file.write_all(b"\n")?;
        }
        file.flush()?;
        file.sync_data()
    }

    fn rotate(&mut self, today: String) -> std::io::Result<()> {
        self.file = None;
        self.day = today.clone();
        let path = self.dir.join(format!("events-{today}.jsonl"));
        self.file = Some(
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)?,
        );
        self.prune_old()?;
        Ok(())
    }

    pub fn prune_old(&self) -> std::io::Result<()> {
        let entries = std::fs::read_dir(&self.dir)?;
        let keep_from = Self::retention_floor_date();
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let Some(date) = name
                .strip_prefix("events-")
                .and_then(|rest| rest.strip_suffix(".jsonl"))
            else {
                continue;
            };
            if date < keep_from.as_str() {
                std::fs::remove_file(entry.path())?;
            }
        }
        Ok(())
    }

    /// Remove account-linked legacy event rows before completing an account
    /// erasure. Session-only rows remain under the existing 90-day retention.
    pub fn erase_account_id(&mut self, account_id: &str) -> std::io::Result<u64> {
        self.erase_account_and_subjects(account_id, &[])
    }

    pub fn erase_account_and_subjects(
        &mut self,
        account_id: &str,
        subject_ids: &[String],
    ) -> std::io::Result<u64> {
        if account_id.len() != 32 || !account_id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid account_id",
            ));
        }
        self.erase_matching(|event| {
            event.get("account_id").and_then(serde_json::Value::as_str) == Some(account_id)
                || event
                    .get("subject_id")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|subject| subject_ids.iter().any(|id| id == subject))
        })
    }

    /// Remove rows linked to channel-scoped pseudonymous subjects during
    /// verified account deletion.
    pub fn erase_subject_ids(&mut self, subject_ids: &[String]) -> std::io::Result<u64> {
        self.erase_matching(|event| {
            event
                .get("subject_id")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|subject| subject_ids.iter().any(|id| id == subject))
        })
    }

    fn erase_matching(
        &mut self,
        matches: impl Fn(&serde_json::Value) -> bool,
    ) -> std::io::Result<u64> {
        self.file = None;
        let entries = std::fs::read_dir(&self.dir)?;
        let files = entries
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        let mut removed = 0_u64;
        for path in files {
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if !name.starts_with("events-") || !name.ends_with(".jsonl") {
                continue;
            }
            removed = removed.saturating_add(scrub_event_file(&path, &matches)?);
        }
        self.rotate(utc_date_string())?;
        Ok(removed)
    }

    fn retention_floor_date() -> String {
        // Walk back RETENTION_DAYS from the current UTC date using the same
        // pure calendar math as time_util (convert via day arithmetic).
        let dt = crate::time_util::now_utc();
        let days = days_from_civil(dt.year, dt.month, dt.day);
        let (year, month, day) = civil_from_days(days - RETENTION_DAYS as i64);
        format!("{year:04}-{month:02}-{day:02}")
    }
}

fn scrub_event_file(
    path: &Path,
    matches: &impl Fn(&serde_json::Value) -> bool,
) -> std::io::Result<u64> {
    let metadata = std::fs::metadata(path)?;
    let temp_path = path.with_extension("jsonl.tmp");
    let result = (|| {
        let input = std::fs::File::open(path)?;
        let output = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temp_path)?;
        let mut output = BufWriter::new(output);
        let mut removed = 0_u64;
        for line in BufReader::new(input).lines() {
            let line = line?;
            let event: serde_json::Value = serde_json::from_str(&line)
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
            if matches(&event) {
                removed = removed.saturating_add(1);
            } else {
                writeln!(output, "{line}")?;
            }
        }
        if removed == 0 {
            return Ok((removed, false));
        }
        let output = output
            .into_inner()
            .map_err(|error| error.into_error())?;
        output.sync_all()?;
        std::fs::set_permissions(&temp_path, metadata.permissions())?;
        std::fs::rename(&temp_path, path)?;
        Ok((removed, true))
    })();
    match result {
        Ok((removed, changed)) => {
            if !changed {
                let _ = std::fs::remove_file(&temp_path);
            }
            Ok(removed)
        }
        Err(error) => {
            let _ = std::fs::remove_file(&temp_path);
            Err(error)
        }
    }
}

fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = year - if month <= 2 { 1 } else { 0 };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let doe = days - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (y + if m <= 2 { 1 } else { 0 }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW_MS: u64 = 1_800_000_000_000;

    fn sample(name: &str) -> AnalyticsEvent {
        serde_json::from_value(serde_json::json!({
            "v": SCHEMA_VERSION, "event_id": "123e4567-e89b-12d3-a456-426614174000",
            "consented": true, "consent_version": CONSENT_POLICY_VERSION,
            "name": name, "ts_ms": NOW_MS, "session_id": "abc123",
            "portal": "site", "platform": "web"
        }))
        .unwrap()
    }

    #[test]
    fn accepts_known_event() {
        assert!(sample("boot_start").validate(NOW_MS).is_ok());
    }

    #[test]
    fn event_taxonomy_has_no_duplicates() {
        let unique: std::collections::HashSet<_> = EVENT_NAMES.iter().copied().collect();
        assert_eq!(unique.len(), EVENT_NAMES.len());
    }

    #[test]
    fn accepts_optional_context_fields() {
        let mut event = sample("boot_start");
        event.build = Some(String::new());
        event.locale = Some(String::new());
        assert!(event.validate(NOW_MS).is_ok());
    }

    #[test]
    fn rejects_unknown_name_and_bad_version_and_bad_ts() {
        assert_eq!(
            sample("not_an_event").validate(NOW_MS),
            Err("unknown event name")
        );
        let mut bad = sample("boot_start");
        bad.v = 9;
        assert_eq!(bad.validate(NOW_MS), Err("unsupported schema version"));
        let mut old = sample("boot_start");
        old.ts_ms = 999_999_999_999;
        assert_eq!(old.validate(NOW_MS), Err("timestamp out of range"));
    }

    #[test]
    fn rejects_unapproved_and_oversized_props() {
        let mut ev = sample("boot_start");
        ev.props = Some(serde_json::json!(["nope"]));
        assert_eq!(ev.validate(NOW_MS), Err("unsupported event properties"));
        let mut big = sample("boot_start");
        big.props = Some(serde_json::json!({"blob": "x".repeat(MAX_PROPS_BYTES + 1)}));
        assert_eq!(big.validate(NOW_MS), Err("props too large"));
    }

    #[test]
    fn consent_and_channel_are_mandatory() {
        let mut event = sample("boot_start");
        event.consented = false;
        assert_eq!(
            event.validate(NOW_MS),
            Err("analytics consent is missing or outdated")
        );
        let mut event = sample("boot_start");
        event.portal = Some("poki".to_string());
        assert_eq!(event.validate(NOW_MS), Err("unsupported analytics channel"));
    }

    #[test]
    fn only_closed_funnel_properties_are_accepted() {
        let mut event = sample("load_stage");
        event.props = Some(serde_json::json!({"stage": "snapshot_available"}));
        assert!(event.validate(NOW_MS).is_ok());
        event.props = Some(serde_json::json!({"stage": "account_name"}));
        assert_eq!(event.validate(NOW_MS), Err("unsupported event properties"));
    }

    #[test]
    fn tutorial_funnel_accepts_bounded_step_identity_and_actions() {
        let mut event = sample("tutorial_step");
        event.props = Some(serde_json::json!({
            "episode_id": "boudica",
            "step_id": "claim-wilderness",
            "step_index": 3,
            "action": "complete"
        }));
        assert!(event.validate(NOW_MS).is_ok());
        event.props = Some(serde_json::json!({
            "episode_id": "boudica",
            "step_id": "last-step",
            "step_index": 511,
            "action": "start"
        }));
        assert!(event.validate(NOW_MS).is_ok());
        event.props = Some(serde_json::json!({
            "episode_id": "boudica",
            "step_id": "past-end",
            "step_index": 512,
            "action": "start"
        }));
        assert_eq!(event.validate(NOW_MS), Err("unsupported event properties"));
        event.props = Some(serde_json::json!({
            "episode_id": "boudica",
            "step_id": "claim-wilderness",
            "step_index": 3,
            "action": "skipped"
        }));
        assert_eq!(event.validate(NOW_MS), Err("unsupported event properties"));
        event.props = Some(serde_json::json!({
            "episode_id": "boudica",
            "step_id": "claim-wilderness",
            "step_index": 3,
            "action": "start",
            "player_name": "not-allowed"
        }));
        assert_eq!(event.validate(NOW_MS), Err("unsupported event properties"));
    }

    #[test]
    fn accepts_episode_events_and_new_shared_outcomes() {
        let mut event = sample("tutorial_start");
        event.props = Some(serde_json::json!({"episode_id": "boudica"}));
        assert!(event.validate(NOW_MS).is_ok());
        event.name = "lobby_join_failed".into();
        event.props = None;
        assert!(event.validate(NOW_MS).is_ok());
        event.name = "match_loading_start".into();
        assert!(event.validate(NOW_MS).is_ok());
        event.name = "campaign_episode_complete".into();
        event.props = Some(serde_json::json!({"episode_id": "boudica"}));
        assert!(event.validate(NOW_MS).is_ok());
    }

    #[test]
    fn tutorial_exit_accepts_legacy_episode_and_bounded_failed_step() {
        let mut event = sample("tutorial_exit_early");
        event.props = Some(serde_json::json!({"episode_id": "boudica"}));
        assert!(event.validate(NOW_MS).is_ok());
        event.props = Some(serde_json::json!({
            "episode_id": "boudica",
            "step_id": "claim-wilderness",
            "step_index": 3,
            "action": "fail"
        }));
        assert!(event.validate(NOW_MS).is_ok());
        event.props = Some(serde_json::json!({
            "episode_id": "boudica",
            "step_id": "claim-wilderness",
            "step_index": 3,
            "action": "complete"
        }));
        assert_eq!(event.validate(NOW_MS), Err("unsupported event properties"));
        event.props = Some(serde_json::json!({
            "episode_id": "boudica",
            "step_id": "claim-wilderness",
            "step_index": 3,
            "action": "fail",
            "player_name": "not-allowed"
        }));
        assert_eq!(event.validate(NOW_MS), Err("unsupported event properties"));
    }

    #[test]
    fn device_class_is_a_coarse_closed_value() {
        let mut event = sample("boot_start");
        event.device_class = Some("mobile".to_string());
        assert!(event.validate(NOW_MS).is_ok());
        event.device_class = Some("iphone-17-pro".to_string());
        assert_eq!(event.validate(NOW_MS), Err("invalid device_class"));
    }

    #[test]
    fn sink_writes_lines_to_daily_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut sink = EventSink::new(dir.path()).unwrap();
        sink.append_line("{\"a\":1}").unwrap();
        sink.append_line("{\"a\":2}").unwrap();
        let today = utc_date_string();
        let contents =
            std::fs::read_to_string(dir.path().join(format!("events-{today}.jsonl"))).unwrap();
        assert_eq!(contents.lines().count(), 2);
    }

    #[test]
    fn sink_prunes_files_past_retention() {
        let dir = tempfile::tempdir().unwrap();
        let floor = EventSink::retention_floor_date();
        let ancient = "events-2000-01-01.jsonl".to_string();
        std::fs::write(dir.path().join(&ancient), "{}\n").unwrap();
        std::fs::write(dir.path().join(format!("events-{floor}.jsonl")), "{}\n").unwrap();
        let mut sink = EventSink::new(dir.path()).unwrap();
        sink.append_line("{}").unwrap();
        assert!(!dir.path().join(&ancient).exists());
        assert!(dir.path().join(format!("events-{floor}.jsonl")).exists());
    }

    #[test]
    fn civil_roundtrip_matches_input() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(days_from_civil(2026, 8, 25)), (2026, 8, 25));
        assert_eq!(shift_date("2026-08-25", -7).as_deref(), Some("2026-08-18"));
        assert_eq!(shift_date("2026-01-01", -1).as_deref(), Some("2025-12-31"));
    }
}
