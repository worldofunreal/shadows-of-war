//! The account owner on top of the existing Redb database. Profile fields
//! that grow with account history live in keyed event rows, not the hot row.
use super::{AccountKind, LinkedIdentity, PlayerAccount, PlayerDb};
use crate::redb_store::{
    ACCOUNT_EVENTS_TABLE, ACCOUNT_IDENTITIES_TABLE, ACCOUNT_STORE_MIGRATIONS_TABLE,
    PENDING_ACCOUNT_REWARDS_TABLE, PENDING_CRAZYGAMES_SCORES_TABLE, PLAYERS_TABLE,
    PUBLIC_PROFILES_TABLE, VERIFIED_VICTORY_BOARD_TABLE,
};
use redb::{Database, ReadTransaction, ReadableTable, WriteTransaction};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct Event {
    kind: String,
    id: String,
    data: Vec<u8>,
    match_id: Option<String>,
    created_at: u64,
    pending: bool,
}

fn hot_account(account: &PlayerAccount) -> Result<Vec<u8>> {
    let mut value = serde_json::to_value(account)?;
    let profile = value
        .get_mut("profile")
        .and_then(serde_json::Value::as_object_mut)
        .ok_or("account profile is missing")?;
    for key in [
        "reward_receipts",
        "purchase_history",
        "processed_revenuecat_events",
    ] {
        profile.remove(key);
    }
    Ok(serde_json::to_vec(&value)?)
}

fn event_rows(account: &PlayerAccount) -> Result<Vec<Event>> {
    let mut events = Vec::new();
    for (id, receipt) in &account.profile.reward_receipts {
        events.push(Event {
            kind: "reward".into(),
            id: id.clone(),
            data: serde_json::to_vec(receipt)?,
            match_id: Some(receipt.match_id.clone()),
            created_at: receipt.created_at,
            pending: receipt.presented_at.is_none(),
        });
    }
    for (id, purchase) in &account.profile.purchase_history {
        events.push(Event {
            kind: "purchase".into(),
            id: id.clone(),
            data: serde_json::to_vec(purchase)?,
            match_id: None,
            created_at: purchase.updated_at,
            pending: false,
        });
    }
    for id in &account.profile.processed_revenuecat_events {
        events.push(Event {
            kind: "provider".into(),
            id: id.clone(),
            data: b"true".to_vec(),
            match_id: None,
            created_at: account.updated_at,
            pending: false,
        });
    }
    events.sort_by(|left, right| (&left.kind, &left.id).cmp(&(&right.kind, &right.id)));
    Ok(events)
}

fn append_part(out: &mut Vec<u8>, value: &str) -> Result<()> {
    let len = u32::try_from(value.len()).map_err(|_| "account event key is too long")?;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

fn account_prefix(account_id: &str) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    append_part(&mut out, account_id)?;
    Ok(out)
}

fn event_key(account_id: &str, kind: &str, id: &str) -> Result<Vec<u8>> {
    let mut out = account_prefix(account_id)?;
    append_part(&mut out, kind)?;
    append_part(&mut out, id)?;
    Ok(out)
}

fn pending_key(account_id: &str, created_at: u64, id: &str) -> Result<Vec<u8>> {
    let mut out = account_prefix(account_id)?;
    out.extend_from_slice(&u64::MAX.saturating_sub(created_at).to_be_bytes());
    append_part(&mut out, id)?;
    Ok(out)
}

fn prefix_end(prefix: &[u8]) -> Result<Vec<u8>> {
    let mut end = prefix.to_vec();
    for index in (0..end.len()).rev() {
        if end[index] != u8::MAX {
            end[index] += 1;
            end.truncate(index + 1);
            return Ok(end);
        }
    }
    Err("account key prefix has no upper bound".into())
}

fn identity_key(identity: &LinkedIdentity) -> Result<Vec<u8>> {
    let mut key = Vec::new();
    append_part(&mut key, &identity.environment)?;
    append_part(&mut key, &identity.provider)?;
    append_part(&mut key, &identity.external_id)?;
    Ok(key)
}

fn decode_event(value: &[u8]) -> Result<Event> {
    Ok(serde_json::from_slice(value)?)
}

fn attach(account: &mut PlayerAccount, event: Event) -> Result<()> {
    match event.kind.as_str() {
        "reward" => {
            account
                .profile
                .reward_receipts
                .insert(event.id, serde_json::from_slice(&event.data)?);
        }
        "purchase" => {
            account
                .profile
                .purchase_history
                .insert(event.id, serde_json::from_slice(&event.data)?);
        }
        "provider" => {
            account.profile.processed_revenuecat_events.insert(event.id);
        }
        _ => return Err("unknown account event kind".into()),
    }
    Ok(())
}

fn load_base(
    table: &impl ReadableTable<&'static str, &'static [u8]>,
    account_id: &str,
) -> Result<PlayerAccount> {
    let value = table.get(account_id)?.ok_or("Account not found")?;
    Ok(crate::db::parse_account_with_migration(value.value())?)
}

fn load_read(txn: &ReadTransaction, account_id: &str, keys: &[&str]) -> Result<PlayerAccount> {
    let mut account = load_base(&txn.open_table(PLAYERS_TABLE)?, account_id)?;
    let prefix = account_prefix(account_id)?;
    let end = prefix_end(&prefix)?;
    let events = txn.open_table(ACCOUNT_EVENTS_TABLE)?;
    let pending = txn.open_table(PENDING_ACCOUNT_REWARDS_TABLE)?;
    for entry in pending.range(prefix.as_slice()..end.as_slice())?.take(64) {
        let (_, event_key) = entry?;
        let value = events
            .get(event_key.value())?
            .ok_or("pending reward index is stale")?;
        attach(&mut account, decode_event(value.value())?)?;
    }
    for id in keys {
        for kind in ["reward", "purchase", "provider"] {
            let key = event_key(account_id, kind, id)?;
            if let Some(value) = events.get(key.as_slice())? {
                attach(&mut account, decode_event(value.value())?)?;
            }
        }
    }
    Ok(account)
}

fn load_write(txn: &WriteTransaction, account_id: &str, keys: &[&str]) -> Result<PlayerAccount> {
    let mut account = load_base(&txn.open_table(PLAYERS_TABLE)?, account_id)?;
    let prefix = account_prefix(account_id)?;
    let end = prefix_end(&prefix)?;
    let events = txn.open_table(ACCOUNT_EVENTS_TABLE)?;
    let pending = txn.open_table(PENDING_ACCOUNT_REWARDS_TABLE)?;
    for entry in pending.range(prefix.as_slice()..end.as_slice())?.take(64) {
        let (_, event_key) = entry?;
        let value = events
            .get(event_key.value())?
            .ok_or("pending reward index is stale")?;
        attach(&mut account, decode_event(value.value())?)?;
    }
    for id in keys {
        for kind in ["reward", "purchase", "provider"] {
            let key = event_key(account_id, kind, id)?;
            if let Some(value) = events.get(key.as_slice())? {
                attach(&mut account, decode_event(value.value())?)?;
            }
        }
    }
    Ok(account)
}

fn migration_marker_keys(
    db: &Database,
    start: &str,
    end: &str,
    limit: usize,
) -> Result<Vec<String>> {
    let read = db.begin_read()?;
    let migrations = read.open_table(ACCOUNT_STORE_MIGRATIONS_TABLE)?;
    Ok(migrations
        .range(start..end)?
        .take(limit)
        .map(|row| row.map(|(key, _)| key.value().to_string()))
        .collect::<std::result::Result<Vec<_>, _>>()?)
}

fn remove_migration_marker_keys(db: &Database, keys: &[String]) -> Result<()> {
    if keys.is_empty() {
        return Ok(());
    }
    let txn = db.begin_write()?;
    let mut migrations = txn.open_table(ACCOUNT_STORE_MIGRATIONS_TABLE)?;
    for key in keys {
        migrations.remove(key.as_str())?;
    }
    drop(migrations);
    txn.commit()?;
    Ok(())
}

fn cleanup_profile_migration_markers(db: &Database) -> Result<()> {
    const BATCH: usize = 256;
    loop {
        let keys = migration_marker_keys(db, "profile-v1:", "profile-v1;", BATCH)?;
        if keys.is_empty() {
            return Ok(());
        }
        remove_migration_marker_keys(db, &keys)?;
    }
}

fn public_index(account: &PlayerAccount) -> Result<Vec<u8>> {
    Ok(serde_json::to_vec(&crate::profile::PublicProfileIndex {
        account_id: account.id.clone(),
        display_name: account.display_name.clone(),
        kind: format!("{:?}", account.kind),
        verified_wins: account.profile.verified_wins,
        updated_at: account.updated_at,
    })?)
}

fn save_identities(
    txn: &WriteTransaction,
    old: Option<&PlayerAccount>,
    new: &PlayerAccount,
) -> Result<()> {
    let before = old
        .into_iter()
        .flat_map(|account| account.linked_identities.iter())
        .map(identity_key)
        .collect::<Result<BTreeSet<_>>>()?;
    let after = new
        .linked_identities
        .iter()
        .map(identity_key)
        .collect::<Result<BTreeSet<_>>>()?;
    let mut table = txn.open_table(ACCOUNT_IDENTITIES_TABLE)?;
    for key in before.difference(&after) {
        if table
            .get(key.as_slice())?
            .is_some_and(|id| id.value() == new.id.as_str())
        {
            table.remove(key.as_slice())?;
        }
    }
    for key in after {
        let existing = table.get(key.as_slice())?.map(|id| id.value().to_string());
        match existing.as_deref() {
            Some(id) if id != new.id => {
                return Err("provider identity is already linked to another account_id".into());
            }
            Some(_) => {}
            None => {
                table.insert(key.as_slice(), new.id.as_str())?;
            }
        }
    }
    Ok(())
}

fn save_indexes(
    txn: &WriteTransaction,
    old: Option<&PlayerAccount>,
    account: &PlayerAccount,
) -> Result<()> {
    {
        let mut table = txn.open_table(PUBLIC_PROFILES_TABLE)?;
        table.insert(account.id.as_str(), public_index(account)?.as_slice())?;
    }
    {
        let mut table = txn.open_table(PENDING_CRAZYGAMES_SCORES_TABLE)?;
        if account
            .linked_identities
            .iter()
            .any(|identity| identity.provider == "crazygames")
            && account.profile.playgames_sync_revision > account.profile.crazygames_synced_revision
        {
            table.insert(account.id.as_str(), b"1".as_slice())?;
        } else {
            table.remove(account.id.as_str())?;
        }
    }
    let mut board = txn.open_table(VERIFIED_VICTORY_BOARD_TABLE)?;
    if let Some(old) =
        old.filter(|old| old.profile.verified_wins > 0 && old.kind != AccountKind::Bot)
    {
        board
            .remove(crate::db::verified_victory_key(old.profile.verified_wins, &old.id).as_str())?;
    }
    if account.profile.verified_wins > 0 && account.kind != AccountKind::Bot {
        let entry = crate::profile::PublicVictoryLeaderboardEntry {
            rank: 0,
            account_id: account.id.clone(),
            handle: crate::profile::public_handle(&account.display_name, &account.id),
            level: account.profile.level,
            matches_played: account.profile.matches_played,
            wins: account.profile.verified_wins,
        };
        board.insert(
            crate::db::verified_victory_key(account.profile.verified_wins, &account.id).as_str(),
            serde_json::to_vec(&entry)?.as_slice(),
        )?;
    }
    Ok(())
}

fn delete_events_for_account(txn: &WriteTransaction, account_id: &str) -> Result<u32> {
    let mut removed = 0_u32;
    let prefix = account_prefix(account_id)?;
    let end = prefix_end(&prefix)?;
    let mut table = txn.open_table(ACCOUNT_EVENTS_TABLE)?;
    let keys = table
        .range(prefix.as_slice()..end.as_slice())?
        .map(|row| row.map(|(key, _)| key.value().to_vec()))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for key in keys {
        if table.remove(key.as_slice())?.is_some() {
            removed = removed.saturating_add(1);
        }
    }
    drop(table);
    let mut pending = txn.open_table(PENDING_ACCOUNT_REWARDS_TABLE)?;
    let keys = pending
        .range(prefix.as_slice()..end.as_slice())?
        .map(|row| row.map(|(key, _)| key.value().to_vec()))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for key in keys {
        if pending.remove(key.as_slice())?.is_some() {
            removed = removed.saturating_add(1);
        }
    }
    Ok(removed)
}

fn write_event(txn: &WriteTransaction, account_id: &str, event: &Event) -> Result<()> {
    let key = event_key(account_id, &event.kind, &event.id)?;
    let mut events = txn.open_table(ACCOUNT_EVENTS_TABLE)?;
    events.insert(key.as_slice(), serde_json::to_vec(event)?.as_slice())?;
    drop(events);
    let mut pending = txn.open_table(PENDING_ACCOUNT_REWARDS_TABLE)?;
    let index_key = pending_key(account_id, event.created_at, &event.id)?;
    if event.pending {
        pending.insert(index_key.as_slice(), key.as_slice())?;
    } else {
        pending.remove(index_key.as_slice())?;
    }
    Ok(())
}

fn save_account(
    txn: &WriteTransaction,
    old: Option<&PlayerAccount>,
    account: &PlayerAccount,
    replace_events: bool,
) -> Result<()> {
    if replace_events {
        delete_events_for_account(txn, &account.id)?;
    }
    let old_events = old
        .map(event_rows)
        .transpose()?
        .unwrap_or_default()
        .into_iter()
        .map(|event| ((event.kind.clone(), event.id.clone()), event))
        .collect::<BTreeMap<_, _>>();
    let new_events = event_rows(account)?
        .into_iter()
        .map(|event| ((event.kind.clone(), event.id.clone()), event))
        .collect::<BTreeMap<_, _>>();

    if !replace_events {
        for ((kind, id), previous) in &old_events {
            if new_events.contains_key(&(kind.clone(), id.clone())) {
                continue;
            }
            let key = event_key(&account.id, kind, id)?;
            txn.open_table(ACCOUNT_EVENTS_TABLE)?
                .remove(key.as_slice())?;
            if previous.pending {
                let key = pending_key(&account.id, previous.created_at, id)?;
                txn.open_table(PENDING_ACCOUNT_REWARDS_TABLE)?
                    .remove(key.as_slice())?;
            }
        }
    }
    for ((kind, id), event) in &new_events {
        let previous = old_events.get(&(kind.clone(), id.clone()));
        if !replace_events && previous == Some(event) {
            continue;
        }
        let key = event_key(&account.id, kind, id)?;
        if previous.is_none()
            && !replace_events
            && txn
                .open_table(ACCOUNT_EVENTS_TABLE)?
                .get(key.as_slice())?
                .is_some()
        {
            return Err("account event must be loaded before it can be changed".into());
        }
        if let Some(previous) = previous.filter(|previous| {
            previous.pending && (!event.pending || previous.created_at != event.created_at)
        }) {
            let previous_key = pending_key(&account.id, previous.created_at, id)?;
            txn.open_table(PENDING_ACCOUNT_REWARDS_TABLE)?
                .remove(previous_key.as_slice())?;
        }
        write_event(txn, &account.id, event)?;
    }

    let bytes = hot_account(account)?;
    txn.open_table(PLAYERS_TABLE)?
        .insert(account.id.as_str(), bytes.as_slice())?;
    save_identities(txn, old, account)?;
    save_indexes(txn, old, account)?;
    Ok(())
}

pub(super) fn delete_account_rows(txn: &WriteTransaction, account_id: &str) -> Result<(bool, u32)> {
    let account = txn
        .open_table(PLAYERS_TABLE)?
        .get(account_id)?
        .map(|value| crate::db::parse_account_with_migration(value.value()))
        .transpose()?;
    let mut removed = 0_u32;

    if let Some(account) = &account {
        let mut identities = txn.open_table(ACCOUNT_IDENTITIES_TABLE)?;
        for identity in &account.linked_identities {
            let key = identity_key(identity)?;
            if identities
                .get(key.as_slice())?
                .is_some_and(|id| id.value() == account_id)
            {
                if identities.remove(key.as_slice())?.is_some() {
                    removed = removed.saturating_add(1);
                }
            }
        }
        drop(identities);

        if account.profile.verified_wins > 0 && account.kind != AccountKind::Bot {
            let key = crate::db::verified_victory_key(account.profile.verified_wins, account_id);
            if txn
                .open_table(VERIFIED_VICTORY_BOARD_TABLE)?
                .remove(key.as_str())?
                .is_some()
            {
                removed = removed.saturating_add(1);
            }
        }
    }

    removed = removed.saturating_add(delete_events_for_account(txn, account_id)?);
    if txn.open_table(PLAYERS_TABLE)?.remove(account_id)?.is_some() {
        removed = removed.saturating_add(1);
    }
    if txn
        .open_table(PUBLIC_PROFILES_TABLE)?
        .remove(account_id)?
        .is_some()
    {
        removed = removed.saturating_add(1);
    }
    if txn
        .open_table(PENDING_CRAZYGAMES_SCORES_TABLE)?
        .remove(account_id)?
        .is_some()
    {
        removed = removed.saturating_add(1);
    }
    Ok((account.is_some(), removed))
}

impl PlayerDb {
    pub(super) async fn update_accounts_atomic<T, F>(
        &self,
        account_ids: Vec<String>,
        keys: Vec<String>,
        update: F,
    ) -> Result<(T, Vec<PlayerAccount>)>
    where
        T: Send + 'static,
        F: FnOnce(&WriteTransaction, &mut [PlayerAccount]) -> Result<T> + Send + 'static,
    {
        let db = self.clone();
        tokio::task::spawn_blocking(move || {
            let key_refs = keys.iter().map(String::as_str).collect::<Vec<_>>();
            db.update_accounts_atomic_sync(&account_ids, &key_refs, update)
        })
        .await?
    }

    pub(super) fn update_accounts_atomic_sync<T, F>(
        &self,
        account_ids: &[String],
        keys: &[&str],
        update: F,
    ) -> Result<(T, Vec<PlayerAccount>)>
    where
        F: FnOnce(&WriteTransaction, &mut [PlayerAccount]) -> Result<T>,
    {
        let db = self
            .redb_db
            .as_ref()
            .ok_or("account database unavailable")?;
        let unique = account_ids.iter().collect::<BTreeSet<_>>();
        if unique.len() != account_ids.len() {
            return Err("duplicate account in atomic update".into());
        }
        let wait_started = std::time::Instant::now();
        let txn = db.begin_write()?;
        let write_wait = wait_started.elapsed();
        let prepare_started = std::time::Instant::now();
        let mut accounts = account_ids
            .iter()
            .map(|account_id| load_write(&txn, account_id, keys))
            .collect::<Result<Vec<_>>>()?;
        let before = accounts.clone();
        let output = update(&txn, &mut accounts)?;
        for (old, account) in before.iter().zip(&accounts) {
            if old.id != account.id {
                return Err("account mutation cannot change its identity".into());
            }
            if hot_account(old)? != hot_account(account)?
                || event_rows(old)? != event_rows(account)?
            {
                save_account(&txn, Some(old), account, false)?;
            }
        }
        let prepare_elapsed = prepare_started.elapsed();
        let commit_started = std::time::Instant::now();
        txn.commit()?;
        let commit_elapsed = commit_started.elapsed();
        log::info!(
            "[rewards-timing] redb_batch accounts={} write_wait_ms={} prepare_ms={} commit_ms={}",
            accounts.len(),
            write_wait.as_millis(),
            prepare_elapsed.as_millis(),
            commit_elapsed.as_millis()
        );
        Ok((output, accounts))
    }

    pub(super) fn load_account_sync(&self, account_id: &str) -> Result<PlayerAccount> {
        let db = self
            .redb_db
            .as_ref()
            .ok_or("account database unavailable")?;
        load_read(&db.begin_read()?, account_id, &[])
    }

    pub(super) fn account_with_events_sync(
        &self,
        account_id: &str,
        keys: &[&str],
    ) -> Result<PlayerAccount> {
        let db = self
            .redb_db
            .as_ref()
            .ok_or("account database unavailable")?;
        load_read(&db.begin_read()?, account_id, keys)
    }

    pub(super) fn update_account_atomic_sync<F>(
        &self,
        account_id: &str,
        keys: &[&str],
        mut mutate: F,
    ) -> Result<PlayerAccount>
    where
        F: FnMut(&mut PlayerAccount),
    {
        let db = self
            .redb_db
            .as_ref()
            .ok_or("account database unavailable")?;
        let txn = db.begin_write()?;
        let mut account = load_write(&txn, account_id, keys)?;
        let before = account.clone();
        mutate(&mut account);
        if account.id != account_id {
            return Err("account mutation cannot change its identity".into());
        }
        let changed = hot_account(&account)? != hot_account(&before)?
            || event_rows(&account)? != event_rows(&before)?;
        if changed {
            save_account(&txn, Some(&before), &account, false)?;
        }
        txn.commit()?;
        Ok(account)
    }

    pub(super) fn insert_account_sync(
        &self,
        account: PlayerAccount,
    ) -> Result<(PlayerAccount, bool)> {
        let db = self
            .redb_db
            .as_ref()
            .ok_or("account database unavailable")?;
        let txn = db.begin_write()?;
        if txn
            .open_table(PLAYERS_TABLE)?
            .get(account.id.as_str())?
            .is_some()
        {
            let existing = load_write(&txn, &account.id, &[])?;
            txn.commit()?;
            return Ok((existing, false));
        }
        save_account(&txn, None, &account, false)?;
        txn.commit()?;
        Ok((account, true))
    }

    pub(super) fn identity_account_sync(
        &self,
        environment: &str,
        provider: &str,
        external_id: &str,
    ) -> Result<Option<String>> {
        let db = self
            .redb_db
            .as_ref()
            .ok_or("account database unavailable")?;
        let mut key = Vec::new();
        append_part(&mut key, environment)?;
        append_part(&mut key, provider)?;
        append_part(&mut key, external_id)?;
        let txn = db.begin_read()?;
        let table = txn.open_table(ACCOUNT_IDENTITIES_TABLE)?;
        Ok(table.get(key.as_slice())?.map(|id| id.value().to_string()))
    }

    pub(super) fn account_exists_sync(&self, account_id: &str) -> Result<bool> {
        let db = self
            .redb_db
            .as_ref()
            .ok_or("account database unavailable")?;
        let txn = db.begin_read()?;
        Ok(txn.open_table(PLAYERS_TABLE)?.get(account_id)?.is_some())
    }

    pub(super) fn is_bot_account_sync(&self, account_id: &str) -> Result<bool> {
        let db = self
            .redb_db
            .as_ref()
            .ok_or("account database unavailable")?;
        let txn = db.begin_read()?;
        let table = txn.open_table(PLAYERS_TABLE)?;
        let Some(value) = table.get(account_id)? else {
            return Ok(false);
        };
        Ok(crate::db::parse_account_with_migration(value.value())?.kind == AccountKind::Bot)
    }

    pub(super) fn count_human_accounts_sync(&self, account_ids: &[String]) -> Result<usize> {
        let db = self
            .redb_db
            .as_ref()
            .ok_or("account database unavailable")?;
        let txn = db.begin_read()?;
        let table = txn.open_table(PLAYERS_TABLE)?;
        let mut unique = BTreeSet::new();
        let mut unique_count = 0;
        for account_id in account_ids {
            if !unique.insert(account_id.as_str()) {
                continue;
            }
            if let Some(value) = table.get(account_id.as_str())?
                && crate::db::parse_account_with_migration(value.value())?.kind
                    == AccountKind::Human
            {
                unique_count += 1;
            }
        }
        Ok(unique_count)
    }

    pub(super) fn purchase_history_sync(
        &self,
        account_id: &str,
    ) -> Result<Vec<crate::db::PurchaseRecord>> {
        let db = self
            .redb_db
            .as_ref()
            .ok_or("account database unavailable")?;
        let txn = db.begin_read()?;
        let prefix = account_prefix(account_id)?;
        let end = prefix_end(&prefix)?;
        let table = txn.open_table(ACCOUNT_EVENTS_TABLE)?;
        let mut purchases = Vec::new();
        for row in table.range(prefix.as_slice()..end.as_slice())? {
            let (_, value) = row?;
            let event = decode_event(value.value())?;
            if event.kind == "purchase" {
                purchases.push(serde_json::from_slice(&event.data)?);
            }
        }
        purchases.sort_by(|left: &crate::db::PurchaseRecord, right| {
            right.updated_at.cmp(&left.updated_at)
        });
        Ok(purchases)
    }

    pub(super) fn purchase_record_sync(
        &self,
        account_id: &str,
        purchase_id: &str,
    ) -> Result<Option<crate::db::PurchaseRecord>> {
        let db = self
            .redb_db
            .as_ref()
            .ok_or("account database unavailable")?;
        let txn = db.begin_read()?;
        let key = event_key(account_id, "purchase", purchase_id)?;
        let table = txn.open_table(ACCOUNT_EVENTS_TABLE)?;
        let Some(value) = table.get(key.as_slice())? else {
            return Ok(None);
        };
        let event = decode_event(value.value())?;
        if event.kind != "purchase" || event.id != purchase_id {
            return Ok(None);
        }
        Ok(Some(serde_json::from_slice(&event.data)?))
    }

    pub(super) fn all_accounts_sync(&self) -> Result<Vec<PlayerAccount>> {
        let db = self
            .redb_db
            .as_ref()
            .ok_or("account database unavailable")?;
        let txn = db.begin_read()?;
        let rows = {
            let table = txn.open_table(PLAYERS_TABLE)?;
            let mut rows = Vec::new();
            for row in table.iter()? {
                let (key, value) = row?;
                rows.push((key.value().to_string(), value.value().to_vec()));
            }
            rows
        };
        let mut accounts = Vec::with_capacity(rows.len());
        for (id, bytes) in rows {
            let mut account = crate::db::parse_account_with_migration(&bytes)?;
            if account.id != id {
                return Err("stored account key does not match its account ID".into());
            }
            let prefix = account_prefix(&id)?;
            let end = prefix_end(&prefix)?;
            let events = txn.open_table(ACCOUNT_EVENTS_TABLE)?;
            for row in events.range(prefix.as_slice()..end.as_slice())? {
                let (_, value) = row?;
                attach(&mut account, decode_event(value.value())?)?;
            }
            accounts.push(account);
        }
        Ok(accounts)
    }

    pub(super) fn account_batch_sync(
        &self,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<PlayerAccount>> {
        use std::ops::Bound::{Excluded, Unbounded};
        let db = self
            .redb_db
            .as_ref()
            .ok_or("account database unavailable")?;
        let txn = db.begin_read()?;
        let table = txn.open_table(PLAYERS_TABLE)?;
        let mut accounts = Vec::with_capacity(limit);
        let rows = if let Some(after) = after {
            table
                .range::<&str>((Excluded(after), Unbounded))?
                .take(limit)
                .map(|row| {
                    row.map(|(key, value)| (key.value().to_string(), value.value().to_vec()))
                })
                .collect::<std::result::Result<Vec<_>, _>>()?
        } else {
            table
                .iter()?
                .take(limit)
                .map(|row| {
                    row.map(|(key, value)| (key.value().to_string(), value.value().to_vec()))
                })
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        for (id, bytes) in rows {
            let account = crate::db::parse_account_with_migration(&bytes)?;
            if account.id != id {
                return Err("stored account key does not match its account ID".into());
            }
            accounts.push(account);
        }
        Ok(accounts)
    }

    pub(super) fn migration_value_sync(&self, key: &str) -> Result<Option<String>> {
        let db = self
            .redb_db
            .as_ref()
            .ok_or("account database unavailable")?;
        let txn = db.begin_read()?;
        let table = txn.open_table(ACCOUNT_STORE_MIGRATIONS_TABLE)?;
        table
            .get(key)?
            .map(|value| String::from_utf8(value.value().to_vec()))
            .transpose()
            .map_err(Into::into)
    }

    pub(super) fn set_migration_value_sync(&self, key: &str, value: &[u8]) -> Result<()> {
        let db = self
            .redb_db
            .as_ref()
            .ok_or("account database unavailable")?;
        let txn = db.begin_write()?;
        txn.open_table(ACCOUNT_STORE_MIGRATIONS_TABLE)?
            .insert(key, value)?;
        txn.commit()?;
        Ok(())
    }

    pub(super) fn delete_account_sync(&self, account_id: &str) -> Result<(bool, u32)> {
        let db = self
            .redb_db
            .as_ref()
            .ok_or("account database unavailable")?;
        let txn = db.begin_write()?;
        let (found, removed) = delete_account_rows(&txn, account_id)?;
        txn.commit()?;
        Ok((found, removed))
    }

    pub(super) fn migrate_legacy_redb_profiles(&self) -> Result<()> {
        use std::ops::Bound::{Excluded, Unbounded};
        const COMPLETE: &str = "schema:account-events-v1";
        const BATCH: usize = 256;
        let db = self
            .redb_db
            .as_ref()
            .ok_or("account database unavailable")?;
        if db
            .begin_read()?
            .open_table(ACCOUNT_STORE_MIGRATIONS_TABLE)?
            .get(COMPLETE)?
            .is_some()
        {
            // Completion is committed before marker cleanup. If a previous
            // process stopped during cleanup, resume it without replaying
            // already-normalized account rows.
            cleanup_profile_migration_markers(db)?;
            return Ok(());
        }
        let mut after: Option<String> = None;
        loop {
            let read = db.begin_read()?;
            let table = read.open_table(PLAYERS_TABLE)?;
            let rows = if let Some(after) = after.as_deref() {
                table
                    .range::<&str>((Excluded(after), Unbounded))?
                    .take(BATCH)
                    .map(|row| {
                        row.map(|(key, value)| (key.value().to_string(), value.value().to_vec()))
                    })
                    .collect::<std::result::Result<Vec<_>, _>>()?
            } else {
                table
                    .iter()?
                    .take(BATCH)
                    .map(|row| {
                        row.map(|(key, value)| (key.value().to_string(), value.value().to_vec()))
                    })
                    .collect::<std::result::Result<Vec<_>, _>>()?
            };
            drop(table);
            drop(read);
            if rows.is_empty() {
                break;
            }

            let txn = db.begin_write()?;
            let mut migrations = txn.open_table(ACCOUNT_STORE_MIGRATIONS_TABLE)?;
            for (id, bytes) in &rows {
                let marker = format!("profile-v1:{id}");
                if migrations.get(marker.as_str())?.is_some() {
                    continue;
                }
                let account = crate::db::parse_account_with_migration(bytes)?;
                if account.id != *id {
                    return Err("stored account key does not match its account ID".into());
                }
                save_account(&txn, Some(&account), &account, true)?;
                migrations.insert(marker.as_str(), b"1".as_slice())?;
            }
            drop(migrations);
            after = rows.last().map(|(id, _)| id.clone());
            txn.commit()?;
        }

        // Keep per-account markers until every legacy row has been converted.
        // A crash before this transaction is safe to resume because committed
        // rows retain their markers in the same transaction as their events.
        let txn = db.begin_write()?;
        txn.open_table(ACCOUNT_STORE_MIGRATIONS_TABLE)?
            .insert(COMPLETE, b"1".as_slice())?;
        txn.commit()?;
        cleanup_profile_migration_markers(db)?;
        Ok(())
    }

    pub(super) async fn load_account(&self, account_id: &str) -> Result<PlayerAccount> {
        self.load_account_sync(account_id)
    }

    pub async fn account_with_events(
        &self,
        account_id: &str,
        keys: &[&str],
    ) -> std::result::Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        self.account_with_events_sync(account_id, keys)
    }

    pub(super) async fn update_account_atomic<F>(
        &self,
        account_id: &str,
        keys: &[&str],
        mutate: F,
    ) -> Result<PlayerAccount>
    where
        F: FnMut(&mut PlayerAccount),
    {
        self.update_account_atomic_sync(account_id, keys, mutate)
    }

    pub(super) async fn insert_account(
        &self,
        account: PlayerAccount,
    ) -> Result<(PlayerAccount, bool)> {
        self.insert_account_sync(account)
    }

    pub(super) async fn identity_account(
        &self,
        environment: &str,
        provider: &str,
        external_id: &str,
    ) -> Result<Option<String>> {
        self.identity_account_sync(environment, provider, external_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACCOUNT_ID: &str = "0123456789abcdef0123456789abcdef";

    fn account_with_receipt() -> PlayerAccount {
        let mut account = PlayerAccount {
            id: ACCOUNT_ID.to_string(),
            display_name: "Migration test".to_string(),
            profile: crate::db::PlayerProfile::default(),
            linked_identities: Vec::new(),
            kind: AccountKind::Human,
            auth_secret_hash: None,
            created_at: 1,
            updated_at: 2,
        };
        account.profile.reward_receipts.insert(
            "match-1".to_string(),
            crate::profile::RewardReceipt {
                id: "match-1".to_string(),
                match_id: "match-1".to_string(),
                created_at: 3,
                ..Default::default()
            },
        );
        account
    }

    fn player_db(path: &std::path::Path) -> (PlayerDb, std::sync::Arc<Database>) {
        let db = std::sync::Arc::new(crate::redb_store::init_database(path).unwrap());
        let player_db = PlayerDb::new("redis://127.0.0.1/", None, Some(db.clone()));
        (player_db, db)
    }

    #[test]
    fn legacy_profile_migration_resumes_without_replacing_saved_events() {
        let temp = tempfile::tempdir().unwrap();
        let (player_db, db) = player_db(&temp.path().join("accounts.redb"));
        let account = account_with_receipt();
        {
            let txn = db.begin_write().unwrap();
            let bytes = serde_json::to_vec(&account).unwrap();
            txn.open_table(PLAYERS_TABLE)
                .unwrap()
                .insert(ACCOUNT_ID, bytes.as_slice())
                .unwrap();
            txn.commit().unwrap();
        }

        player_db.migrate_legacy_redb_profiles().unwrap();
        let migrated = player_db
            .account_with_events_sync(ACCOUNT_ID, &["match-1"])
            .unwrap();
        assert!(migrated.profile.reward_receipts.contains_key("match-1"));

        // Model a process that committed conversion but stopped before
        // cleaning the per-account marker.
        {
            let txn = db.begin_write().unwrap();
            txn.open_table(ACCOUNT_STORE_MIGRATIONS_TABLE)
                .unwrap()
                .insert(format!("profile-v1:{ACCOUNT_ID}").as_str(), b"1".as_slice())
                .unwrap();
            txn.commit().unwrap();
        }
        player_db.migrate_legacy_redb_profiles().unwrap();

        let migrated = player_db
            .account_with_events_sync(ACCOUNT_ID, &["match-1"])
            .unwrap();
        assert!(migrated.profile.reward_receipts.contains_key("match-1"));
        assert!(
            db.begin_read()
                .unwrap()
                .open_table(ACCOUNT_STORE_MIGRATIONS_TABLE)
                .unwrap()
                .get(format!("profile-v1:{ACCOUNT_ID}").as_str())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn legacy_profile_migration_resumes_after_a_committed_batch() {
        let temp = tempfile::tempdir().unwrap();
        let (player_db, db) = player_db(&temp.path().join("accounts.redb"));
        let account = account_with_receipt();
        {
            let txn = db.begin_write().unwrap();
            save_account(&txn, None, &account, true).unwrap();
            txn.open_table(ACCOUNT_STORE_MIGRATIONS_TABLE)
                .unwrap()
                .insert(format!("profile-v1:{ACCOUNT_ID}").as_str(), b"1".as_slice())
                .unwrap();
            txn.commit().unwrap();
        }

        player_db.migrate_legacy_redb_profiles().unwrap();
        let migrated = player_db
            .account_with_events_sync(ACCOUNT_ID, &["match-1"])
            .unwrap();
        assert!(migrated.profile.reward_receipts.contains_key("match-1"));
        assert!(
            db.begin_read()
                .unwrap()
                .open_table(ACCOUNT_STORE_MIGRATIONS_TABLE)
                .unwrap()
                .get("schema:account-events-v1")
                .unwrap()
                .is_some()
        );
    }
}
