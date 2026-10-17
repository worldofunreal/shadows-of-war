//! Static game data, player database, and embedded REDB metadata.

pub const MAX_MATCH_PARTICIPANTS: usize = 256;
/// Shared relay/verifier limits, derived from the registered simulation clock.
pub const MAX_MATCH_SECONDS: u64 = 4 * 60 * 60;
pub const MATCH_PROGRESS_INTERVAL_SECONDS: u64 = 5;
pub const MAX_MATCH_CHUNK_BYTES: usize = 1024 * 1024;
pub const MAX_MATCH_ARCHIVE_BYTES: usize = 256 * 1024 * 1024;

pub fn max_match_turns(tick_rate_ms: f32) -> u64 {
    ((MAX_MATCH_SECONDS as f64 * 1000.0) / f64::from(tick_rate_ms)).ceil() as u64 + 2
}

pub mod colors;
pub mod campaign;
pub mod commerce;
#[cfg(feature = "server")]
pub mod crazygames;
#[cfg(feature = "server")]
pub mod db;
pub mod emoji;
#[cfg(feature = "server")]
pub mod events;
pub mod geo_entities;
pub mod leaders;
#[cfg(feature = "server")]
pub mod metadata_db;
#[cfg(feature = "server")]
pub mod moderation;
pub mod name_policy;
pub mod profile;
pub mod rewards;
#[cfg(feature = "server")]
pub mod time_util;
pub mod tribes;

pub use colors::{NamedColor, PREMIUM_COLORS};
pub use commerce::{LeaderResolution, StoreCatalog};
#[cfg(feature = "server")]
pub use db::{LinkedIdentity, PlayerAccount, PlayerDb, PlayerProfile};
pub use emoji::{ATLAS_HEIGHT, ATLAS_WIDTH, AtlasRect, lookup};
pub use leaders::{Civilization, Leader, leader_for_civilization};
#[cfg(feature = "server")]
pub use metadata_db::{
    GeoEntityRecord, LeaderRecord, get_geo_entity_by_name, get_leader_by_name, init_database,
};
pub use profile::{
    MatchParticipantRecord, MatchRecord, PublicLeaderSummary, PublicLeaderboardEntry,
    PublicMatchDetail, PublicMatchSummary, PublicProfileIndex, PublicProfileSummary,
    PublicProfileView, PublicRatingView, PublicVictoryLeaderboardEntry, SeasonRating, SeasonRecord,
};
pub use tribes::{
    EMPIRE_EMOJIS, TRIBE_ANIMALS, animal_for_id, animal_for_name, empire_emoji_for_id,
    empire_emoji_for_name, fallback_nations, fallback_tribes,
};
