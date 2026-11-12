use redb::{Database, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};
use std::path::Path;

pub const PLAYERS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("players");
pub const ACCOUNT_EVENTS_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("account_events");
pub const ACCOUNT_IDENTITIES_TABLE: TableDefinition<&[u8], &str> =
    TableDefinition::new("account_identities");
pub const PENDING_ACCOUNT_REWARDS_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("pending_account_rewards");
pub const ACCOUNT_STORE_MIGRATIONS_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("account_store_migrations");
pub const MATCHES_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("match_records");
pub const PLAYER_MATCH_INDEX_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("player_match_index");
pub const SEASON_RATINGS_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("season_ratings");
pub const RANKED_BOTS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("ranked_bots");
pub const SEASONS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("seasons");
pub const PUBLIC_PROFILES_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("public_profiles");
pub const MATCH_STARTS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("match_starts");
pub const PENDING_REPLAY_VERIFICATIONS_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("pending_replay_verifications");
pub const VERIFIED_VICTORY_BOARD_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("verified_victory_board");
pub const PENDING_CRAZYGAMES_SCORES_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("pending_crazygames_scores");
pub const HERO_ROTATIONS_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("hero_rotations");

const HERO_ROTATION_STATE_KEY: &str = "state";

#[derive(Serialize, Deserialize)]
struct HeroRotationRecord {
    leaders: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct QueuedHero {
    id: String,
    eligible_period: u64,
}

#[derive(Serialize, Deserialize)]
struct HeroRotationState {
    latest_period: u64,
    known_ids: BTreeSet<String>,
    queued: Vec<QueuedHero>,
}

/// Automatically initialize and build the redb metadata database file if it does not exist
pub fn init_database<P: AsRef<Path>>(path: P) -> Result<Database, Box<dyn std::error::Error>> {
    if let Some(parent) = path.as_ref().parent()
        && !parent.exists()
    {
        std::fs::create_dir_all(parent)?;
    }
    let db = Database::create(path)?;
    // Opening each table creates it on a new install and leaves data alone on an existing one.
    let score_index_exists = db
        .begin_read()?
        .open_table(PENDING_CRAZYGAMES_SCORES_TABLE)
        .is_ok();
    let write_txn = db.begin_write()?;
    write_txn.open_table(PLAYERS_TABLE)?;
    write_txn.open_table(crate::auth::SESSIONS)?;
    write_txn.open_table(ACCOUNT_EVENTS_TABLE)?;
    write_txn.open_table(ACCOUNT_IDENTITIES_TABLE)?;
    write_txn.open_table(PENDING_ACCOUNT_REWARDS_TABLE)?;
    write_txn.open_table(ACCOUNT_STORE_MIGRATIONS_TABLE)?;
    write_txn.open_table(MATCHES_TABLE)?;
    write_txn.open_table(PLAYER_MATCH_INDEX_TABLE)?;
    write_txn.open_table(SEASON_RATINGS_TABLE)?;
    write_txn.open_table(RANKED_BOTS_TABLE)?;
    write_txn.open_table(SEASONS_TABLE)?;
    write_txn.open_table(PUBLIC_PROFILES_TABLE)?;
    write_txn.open_table(MATCH_STARTS_TABLE)?;
    write_txn.open_table(PENDING_REPLAY_VERIFICATIONS_TABLE)?;
    write_txn.open_table(VERIFIED_VICTORY_BOARD_TABLE)?;
    write_txn.open_table(PENDING_CRAZYGAMES_SCORES_TABLE)?;
    write_txn.open_table(HERO_ROTATIONS_TABLE)?;
    if !score_index_exists {
        let players = write_txn.open_table(PLAYERS_TABLE)?;
        let mut pending = write_txn.open_table(PENDING_CRAZYGAMES_SCORES_TABLE)?;
        for row in players.iter()? {
            let (key, bytes) = row?;
            let account: crate::db::PlayerAccount = serde_json::from_slice(bytes.value())?;
            if account
                .linked_identities
                .iter()
                .any(|identity| identity.provider == "crazygames")
                && account.profile.playgames_sync_revision
                    > account.profile.crazygames_synced_revision
            {
                pending.insert(key.value(), b"1".as_slice())?;
            }
        }
    }
    write_txn.commit()?;
    Ok(db)
}

/// Return the durable rotation for this week, creating each missing week once.
/// The first call freezes the lineup the old server would have shown today.
pub fn current_free_hero_rotation(
    db: &Database,
) -> Result<Vec<crate::leaders::Leader>, Box<dyn std::error::Error + Send + Sync>> {
    free_hero_rotation_for_period(db, crate::commerce::current_rotation_period())
}

pub(crate) fn free_hero_rotation_for_period(
    db: &Database,
    current_period: u64,
) -> Result<Vec<crate::leaders::Leader>, Box<dyn std::error::Error + Send + Sync>> {
    use crate::commerce::{
        FREE_ROTATION_SIZE, generate_free_rotation, leader_from_any_id, leader_id,
        legacy_rotation_for_period, repair_free_rotation,
    };

    use crate::leaders::Leader;
    let transaction = db.begin_write()?;
    let mut table = transaction.open_table(HERO_ROTATIONS_TABLE)?;
    let raw_state = table
        .get(HERO_ROTATION_STATE_KEY)?
        .map(|value| value.value().to_vec());

    let mut state = if let Some(raw) = raw_state {
        serde_json::from_slice::<HeroRotationState>(&raw)?
    } else {
        let rotation = legacy_rotation_for_period(current_period);
        if rotation.len() != FREE_ROTATION_SIZE {
            return Err(
                "cannot freeze the active hero rotation: catalog has fewer than eight candidates"
                    .into(),
            );
        }
        let row_key = format!("period:{current_period}");
        let record = HeroRotationRecord {
            leaders: rotation
                .iter()
                .map(|leader| leader_id(*leader).to_string())
                .collect(),
        };
        table.insert(row_key.as_str(), serde_json::to_vec(&record)?.as_slice())?;
        HeroRotationState {
            latest_period: current_period,
            known_ids: Leader::all()
                .map(|leader| leader_id(leader).to_string())
                .collect(),
            queued: Vec::new(),
        }
    };

    state
        .queued
        .retain(|queued| leader_from_any_id(&queued.id).is_some_and(Leader::is_enabled));

    let known_now = Leader::all()
        .map(|leader| leader_id(leader).to_string())
        .collect::<BTreeSet<_>>();
    let next_period = current_period.saturating_add(1);
    for leader in Leader::all().filter(|leader| !leader.always_free()) {
        let id = leader_id(leader).to_string();
        if !state.known_ids.contains(&id) && !state.queued.iter().any(|queued| queued.id == id) {
            state.queued.push(QueuedHero {
                id,
                eligible_period: next_period,
            });
        }
    }
    state.known_ids = known_now;

    while state.latest_period < current_period {
        let previous_key = format!("period:{}", state.latest_period);
        let previous = {
            let record = table
                .get(previous_key.as_str())?
                .ok_or("latest hero rotation record is missing")?;
            let record = serde_json::from_slice::<HeroRotationRecord>(record.value())?;
            record
                .leaders
                .iter()
                .map(|id| {
                    leader_from_any_id(id)
                        .ok_or_else(|| format!("unknown stored hero id: {id}"))
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        let period = state.latest_period.saturating_add(1);
        let candidates = Leader::all()
            .filter(|leader| !leader.always_free())
            .filter(|leader| {
                !state.queued.iter().any(|queued| {
                    queued.id == leader_id(*leader) && queued.eligible_period > period
                })
            })
            .collect::<Vec<_>>();
        let queued = state
            .queued
            .iter()
            .filter(|queued| queued.eligible_period <= period)
            .filter_map(|queued| leader_from_any_id(&queued.id))
            .collect::<Vec<_>>();
        let (rotation, _) =
            generate_free_rotation(&candidates, &previous, &queued, &mut rand::thread_rng())
                .ok_or("unable to satisfy hero rotation constraints")?;
        let selected = rotation
            .iter()
            .map(|leader| leader_id(*leader).to_string())
            .collect::<HashSet<_>>();
        state.queued.retain(|queued| !selected.contains(&queued.id));
        state.latest_period = period;
        let key = format!("period:{period}");
        let record = HeroRotationRecord {
            leaders: rotation
                .iter()
                .map(|leader| leader_id(*leader).to_string())
                .collect(),
        };
        table.insert(key.as_str(), serde_json::to_vec(&record)?.as_slice())?;
    }

    let current_key = format!("period:{current_period}");
    let rotation = {
        let record = table
            .get(current_key.as_str())?
            .ok_or("current hero rotation record is missing")?;
        let record = serde_json::from_slice::<HeroRotationRecord>(record.value())?;
        record
            .leaders
            .iter()
            .map(|id| leader_from_any_id(id).ok_or_else(|| format!("unknown stored hero id: {id}")))
            .collect::<Result<Vec<_>, _>>()?
    };

    let active_rotation = rotation
        .iter()
        .copied()
        .filter(|leader| leader.is_enabled())
        .collect::<Vec<_>>();
    let unique_rotation = active_rotation.iter().copied().collect::<HashSet<_>>();
    let rotation = if active_rotation.len() == FREE_ROTATION_SIZE
        && unique_rotation.len() == FREE_ROTATION_SIZE
    {
        active_rotation
    } else {
        let previous = if current_period == 0 {
            Vec::new()
        } else {
            let previous_key = format!("period:{}", current_period - 1);
            match table.get(previous_key.as_str())? {
                Some(record) => serde_json::from_slice::<HeroRotationRecord>(record.value())?
                    .leaders
                    .iter()
                    .map(|id| {
                        leader_from_any_id(id)
                            .ok_or_else(|| format!("unknown stored hero id: {id}"))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                None => Vec::new(),
            }
        };
        let candidates = Leader::all()
            .filter(|leader| !leader.always_free())
            .filter(|leader| {
                !state.queued.iter().any(|queued| {
                    queued.id == leader_id(*leader) && queued.eligible_period > current_period
                })
            })
            .collect::<Vec<_>>();
        let repaired = repair_free_rotation(
            &candidates,
            &previous,
            &active_rotation,
            &mut rand::thread_rng(),
        )
        .ok_or("unable to repair the current hero rotation")?;
        if repaired.len() != FREE_ROTATION_SIZE {
            return Err("repaired hero rotation does not contain eight heroes".into());
        }
        state.queued.retain(|queued| {
            !repaired.iter().any(|leader| leader_id(*leader) == queued.id)
        });
        let record = HeroRotationRecord {
            leaders: repaired
                .iter()
                .map(|leader| leader_id(*leader).to_string())
                .collect(),
        };
        table.insert(current_key.as_str(), serde_json::to_vec(&record)?.as_slice())?;
        repaired
    };
    table.insert(
        HERO_ROTATION_STATE_KEY,
        serde_json::to_vec(&state)?.as_slice(),
    )?;
    drop(table);
    transaction.commit()?;
    Ok(rotation)
}

#[cfg(test)]
mod hero_rotation_tests {
    use super::*;
    use crate::commerce::{
        FREE_ROTATION_SIZE, generate_free_rotation, leader_from_id, leader_id,
        legacy_rotation_for_period,
    };
    use crate::geo_entities::{Gender, Region};
    use rand::SeedableRng;
    use std::sync::Arc;

    #[test]
    fn weekly_rotation_is_frozen_across_calls_parallel_reads_and_restart() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("rotations.redb");
        let period = 9_000;
        let database = init_database(&path).unwrap();
        let initial = free_hero_rotation_for_period(&database, period).unwrap();
        assert_eq!(initial, legacy_rotation_for_period(period));
        assert_eq!(
            free_hero_rotation_for_period(&database, period).unwrap(),
            initial
        );

        let database = Arc::new(database);
        std::thread::scope(|scope| {
            let requests = (0..8)
                .map(|_| {
                    let database = Arc::clone(&database);
                    scope.spawn(move || free_hero_rotation_for_period(&database, period).unwrap())
                })
                .collect::<Vec<_>>();
            for request in requests {
                assert_eq!(request.join().unwrap(), initial);
            }
        });
        drop(database);

        let database = init_database(&path).unwrap();
        assert_eq!(
            free_hero_rotation_for_period(&database, period).unwrap(),
            initial
        );
    }

    #[test]
    fn current_rotation_repairs_a_missing_hero_and_persists_the_replacement() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("rotation-repair.redb");
        let period = 9_050;
        let database = init_database(&path).unwrap();
        free_hero_rotation_for_period(&database, period).unwrap();
        let candidates = crate::leaders::Leader::all()
            .filter(|leader| !leader.always_free())
            .collect::<Vec<_>>();
        let (original, _) = generate_free_rotation(
            &candidates,
            &[],
            &[],
            &mut rand::rngs::StdRng::seed_from_u64(9050),
        )
        .unwrap();
        let disabled = crate::leaders::Leader::all_known().find(|leader| !leader.is_enabled());
        let mut persisted = original.clone();
        if let Some(disabled) = disabled {
            persisted[0] = disabled;
        } else {
            persisted.remove(0);
        }
        let retained = original.iter().copied().skip(1).collect::<Vec<_>>();

        let transaction = database.begin_write().unwrap();
        {
            let mut table = transaction.open_table(HERO_ROTATIONS_TABLE).unwrap();
            let key = format!("period:{period}");
            let record = HeroRotationRecord {
                leaders: persisted
                    .iter()
                    .map(|leader| leader_id(*leader).to_string())
                    .collect(),
            };
            table
                .insert(key.as_str(), serde_json::to_vec(&record).unwrap().as_slice())
                .unwrap();
        }
        transaction.commit().unwrap();

        let repaired = free_hero_rotation_for_period(&database, period).unwrap();
        assert_eq!(repaired.len(), FREE_ROTATION_SIZE);
        assert!(retained.iter().all(|leader| repaired.contains(leader)));
        assert!(repaired.iter().all(|leader| leader.is_enabled()));
        assert_eq!(
            free_hero_rotation_for_period(&database, period).unwrap(),
            repaired
        );
        drop(database);

        let database = init_database(&path).unwrap();
        assert_eq!(
            free_hero_rotation_for_period(&database, period).unwrap(),
            repaired
        );
    }

    #[test]
    fn a_new_catalog_hero_waits_for_the_next_period_and_is_included_there() {
        let directory = tempfile::tempdir().unwrap();
        let database = init_database(directory.path().join("rotation-queue.redb")).unwrap();
        let period = 9_100;
        let active = free_hero_rotation_for_period(&database, period).unwrap();
        let queued = crate::leaders::Leader::all()
            .find(|leader| {
                !leader.always_free()
                    && !active.contains(leader)
                    && leader.character().region == Region::Europe
                    && leader.character().gender == Some(Gender::Female)
            })
            .unwrap();

        let transaction = database.begin_write().unwrap();
        {
            let mut table = transaction.open_table(HERO_ROTATIONS_TABLE).unwrap();
            let mut state = serde_json::from_slice::<HeroRotationState>(
                table.get(HERO_ROTATION_STATE_KEY).unwrap().unwrap().value(),
            )
            .unwrap();
            state.known_ids.remove(leader_id(queued));
            table
                .insert(
                    HERO_ROTATION_STATE_KEY,
                    serde_json::to_vec(&state).unwrap().as_slice(),
                )
                .unwrap();
        }
        transaction.commit().unwrap();

        assert_eq!(
            free_hero_rotation_for_period(&database, period).unwrap(),
            active
        );
        let next = free_hero_rotation_for_period(&database, period + 1).unwrap();
        assert_eq!(next.len(), FREE_ROTATION_SIZE);
        assert!(next.contains(&queued));
        assert!(next.iter().all(|leader| !active.contains(leader)));
        assert!(
            [
                Region::Africa,
                Region::Americas,
                Region::Asia,
                Region::Europe,
                Region::Oceania
            ]
            .into_iter()
            .all(|region| next
                .iter()
                .any(|leader| leader.character().region == region))
        );
        assert_eq!(
            next.iter()
                .filter(|leader| leader.character().gender == Some(Gender::Female))
                .count(),
            4
        );
        assert_eq!(
            next.iter()
                .filter(|leader| leader.character().gender == Some(Gender::Male))
                .count(),
            4
        );
        assert!(leader_from_id(leader_id(queued)).is_some());
    }

    #[test]
    fn queued_batch_overflow_carries_into_later_week() {
        let directory = tempfile::tempdir().unwrap();
        let database = init_database(directory.path().join("rotation-overflow.redb")).unwrap();
        let period = 9_200;
        let active = free_hero_rotation_for_period(&database, period).unwrap();
        let candidates = crate::leaders::Leader::all()
            .filter(|leader| !leader.always_free() && !active.contains(leader))
            .collect::<Vec<_>>();
        let mut added = Vec::new();
        for region in [
            Region::Africa,
            Region::Americas,
            Region::Asia,
            Region::Europe,
            Region::Oceania,
        ] {
            added.push(
                candidates
                    .iter()
                    .copied()
                    .find(|leader| {
                        leader.character().region == region
                            && leader.character().gender == Some(Gender::Female)
                    })
                    .unwrap(),
            );
        }
        for region in [
            Region::Africa,
            Region::Americas,
            Region::Asia,
            Region::Europe,
        ] {
            added.push(
                candidates
                    .iter()
                    .copied()
                    .find(|leader| {
                        leader.character().region == region
                            && leader.character().gender == Some(Gender::Male)
                    })
                    .unwrap(),
            );
        }
        assert_eq!(added.len(), 9);

        let transaction = database.begin_write().unwrap();
        {
            let mut table = transaction.open_table(HERO_ROTATIONS_TABLE).unwrap();
            let mut state = serde_json::from_slice::<HeroRotationState>(
                table.get(HERO_ROTATION_STATE_KEY).unwrap().unwrap().value(),
            )
            .unwrap();
            for leader in &added {
                state.known_ids.remove(leader_id(*leader));
            }
            table
                .insert(
                    HERO_ROTATION_STATE_KEY,
                    serde_json::to_vec(&state).unwrap().as_slice(),
                )
                .unwrap();
        }
        transaction.commit().unwrap();

        assert_eq!(
            free_hero_rotation_for_period(&database, period).unwrap(),
            active
        );
        let next = free_hero_rotation_for_period(&database, period + 1).unwrap();
        assert_eq!(next.len(), FREE_ROTATION_SIZE);
        assert_eq!(
            next.iter().filter(|leader| added.contains(leader)).count(),
            8
        );
        assert!(next.iter().all(|leader| !active.contains(leader)));
        let pending_id = {
            let read = database.begin_read().unwrap();
            let table = read.open_table(HERO_ROTATIONS_TABLE).unwrap();
            let state = serde_json::from_slice::<HeroRotationState>(
                table.get(HERO_ROTATION_STATE_KEY).unwrap().unwrap().value(),
            )
            .unwrap();
            assert_eq!(state.queued.len(), 1);
            state.queued[0].id.clone()
        };
        let pending = leader_from_id(&pending_id).unwrap();
        assert!(added.contains(&pending));
        let following = free_hero_rotation_for_period(&database, period + 2).unwrap();
        assert!(following.contains(&pending));
        assert!(following.iter().all(|leader| !next.contains(leader)));
    }
}
