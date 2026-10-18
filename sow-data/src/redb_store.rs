use redb::{Database, ReadableTable, TableDefinition};
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
    write_txn.open_table(ACCOUNT_EVENTS_TABLE)?;
    write_txn.open_table(ACCOUNT_IDENTITIES_TABLE)?;
    write_txn.open_table(PENDING_ACCOUNT_REWARDS_TABLE)?;
    write_txn.open_table(ACCOUNT_STORE_MIGRATIONS_TABLE)?;
    write_txn.open_table(MATCHES_TABLE)?;
    write_txn.open_table(PLAYER_MATCH_INDEX_TABLE)?;
    write_txn.open_table(SEASON_RATINGS_TABLE)?;
    write_txn.open_table(SEASONS_TABLE)?;
    write_txn.open_table(PUBLIC_PROFILES_TABLE)?;
    write_txn.open_table(MATCH_STARTS_TABLE)?;
    write_txn.open_table(PENDING_REPLAY_VERIFICATIONS_TABLE)?;
    write_txn.open_table(VERIFIED_VICTORY_BOARD_TABLE)?;
    write_txn.open_table(PENDING_CRAZYGAMES_SCORES_TABLE)?;
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
