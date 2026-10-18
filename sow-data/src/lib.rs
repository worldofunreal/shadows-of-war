//! Static game data, player database, and durable Redb storage.

pub const MAX_MATCH_PARTICIPANTS: usize = 256;
/// Shared relay/verifier limits, derived from the registered simulation clock.
pub const MAX_MATCH_SECONDS: u64 = 4 * 60 * 60;
pub const MATCH_PROGRESS_INTERVAL_SECONDS: u64 = 1;
pub const MAX_MATCH_CHUNK_BYTES: usize = 1024 * 1024;
pub const MAX_MATCH_ARCHIVE_BYTES: usize = 256 * 1024 * 1024;

pub fn max_match_turns(tick_rate_ms: f32) -> u64 {
    ((MAX_MATCH_SECONDS as f64 * 1000.0) / f64::from(tick_rate_ms)).ceil() as u64 + 2
}

pub mod campaign;
pub mod colors;
pub mod commerce;
#[cfg(feature = "server")]
pub mod crazygames;
#[cfg(feature = "server")]
pub mod db;
#[cfg(feature = "server")]
pub mod events;
pub mod geo_entities;
pub mod leaders;
#[cfg(feature = "server")]
pub mod moderation;
pub mod name_policy;
pub mod profile;
#[cfg(feature = "server")]
pub mod redb_store;
pub mod rewards;
#[cfg(feature = "server")]
pub mod time_util;

pub use colors::{NamedColor, PREMIUM_COLORS};
pub use commerce::{LeaderResolution, StoreCatalog};
#[cfg(feature = "server")]
pub use db::{LinkedIdentity, PlayerAccount, PlayerDb, PlayerProfile};
pub use leaders::Leader;
pub use profile::{
    MatchParticipantRecord, MatchRecord, PublicLeaderSummary, PublicLeaderboardEntry,
    PublicMatchDetail, PublicMatchSummary, PublicProfileIndex, PublicProfileSummary,
    PublicProfileView, PublicRatingView, PublicVictoryLeaderboardEntry, SeasonRating, SeasonRecord,
};
#[cfg(feature = "server")]
pub use redb_store::init_database;
