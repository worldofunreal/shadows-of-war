//! Fictional humans — internal bot fill for Matchmaking lobbies.
//!
//! ══════════════════════════════════════════════════════════════════════
//! MENTAL MODEL — read this before touching ANYTHING in this module.
//! ══════════════════════════════════════════════════════════════════════
//! These are FICTIONAL HUMANS. Not PvE bots, not a daemon, not "the
//! backfill". They are human players that don't exist, and they must look
//! like it.
//!
//! THE ONE RULE: organic. Nothing here may ever be predictable, regular,
//! or metronomic. A lobby must never fill at a fixed rate. Two lobbies
//! must never fill alike. Nobody — player or agent — may ever be able to
//! guess how many ghosts there are, or when the next one joins.
//!
//! What "organic" looks like in practice:
//!   - each ghost INDEPENDENTLY picks a random moment somewhere across the
//!     whole countdown window (second 0 to the last second). No mean, no
//!     drip, no "next after this one"
//!   - bursts (several ghosts landing on the same moment) are normal —
//!     that's what independent placement produces
//!   - silences (nobody for several seconds) are normal — same reason
//!   - a lobby can fill in the first 2s, in the last 2s, spread out, or
//!     never get past half. Nobody can guess the rhythm because there IS none
//!   - the fill percentage is random per lobby (min/max bounds only)
//!   - there is NO perceptible rate. If a human watching can tap a tempo,
//!     it's broken
//!
//! REGRESSION WARNINGS (for future agents):
//!   - reintroducing a per-lobby mean, a drip cooldown, an inter-arrival
//!     distribution with a central tendency, or ANY sequential "next entry
//!     depends on the previous" structure = REGRESSION. That recreates the
//!     perceptible heartbeat this module exists to kill
//!   - adding a fixed interval, constant pace, linear ramp, deterministic
//!     schedule, or "smoother" distribution = REGRESSION, not a fix
//!   - "fixing" randomness that looks random = REGRESSION
//!   - "helping" by making entry consistent = REGRESSION
//!   - replacing randomness with anything an observer could predict =
//!     REGRESSION.
//!
//! Identities come exclusively from the persistent bot-account pool seeded
//! at boot (`BotPool`) — real `database_account_id`s and stats accumulation.
//! A random share may use lobby-only `Anon####` display names; the pool's
//! stored names and accounts are never changed. If the pool is unavailable,
//! nothing is injected.
//! ══════════════════════════════════════════════════════════════════════

pub mod names;

use crate::lobby::{LOBBY_COUNTDOWN_SECS, LobbyPhase, PlayerConnection, ServerLobby};
use rand::Rng;
use sow_core::player::Leader;
use sow_core::protocol::{LobbyKind, Team};
use std::env;
use std::sync::OnceLock;
use tokio::sync::mpsc;

// ── Persistent bot-account pool ───────────────────────────────────────────

/// One persistent bot identity: a stable account id (resolves to a
/// `PlayerAccount` with `kind = Bot` in sow-data) paired with the display
/// name shown in lobbies and in-game.
///
/// MENTAL MODEL: the pool is just identities. It says NOTHING about when
/// or how many of them join — entry timing lives in the drip below and is
/// deliberately chaotic.
#[derive(Clone, Debug)]
pub struct BotPoolEntry {
    pub account_id: String,
    pub display_name: String,
}

/// The resolved bot pool. Installed once at boot via `BotPool::install`; read
/// from every subsequent `inject_internal_bots` call through the `OnceLock`.
pub struct BotPool {
    entries: Vec<BotPoolEntry>,
}

static BOT_POOL: OnceLock<BotPool> = OnceLock::new();

const ANON_NAME_COUNT: usize = 10_000;

/// Share of a lobby's ghosts shown as `Anon####`, by the ranked league of the
/// humans in it. Basic leagues are mostly Anon; higher leagues have almost none.
/// No rated human (`None`) counts as Bronze.
fn anon_band(league_score: Option<u16>) -> (f32, f32) {
    let (tier, _) = sow_data::profile::tier_for_score(league_score.unwrap_or(0));
    match tier {
        "Bronze" => (0.50, 0.70),
        "Silver" => (0.20, 0.35),
        "Gold" => (0.05, 0.10),
        _ => (0.0, 0.0),
    }
}

fn roll_anon_count(ghost_count: usize, band: (f32, f32), rng: &mut impl Rng) -> usize {
    ((ghost_count as f32 * rng.gen_range(band.0..=band.1)).round() as usize).min(ANON_NAME_COUNT)
}

fn assign_anon_names(identities: &mut [BotPoolEntry], count: usize, rng: &mut impl Rng) {
    let mut used = std::collections::HashSet::with_capacity(count);
    for identity in identities.iter_mut().take(count) {
        loop {
            let name = format!("Anon{:04}", rng.gen_range(0..ANON_NAME_COUNT));
            if used.insert(name.clone()) {
                identity.display_name = name;
                break;
            }
        }
    }
}

impl BotPool {
    pub fn new(entries: Vec<BotPoolEntry>) -> Self {
        Self { entries }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Publish this pool as the process-wide bot identity source. Called once
    /// at boot from `main`; subsequent calls are silently ignored (the first
    /// one wins), which keeps boot idempotent if init is retried.
    pub fn install(self) {
        let n = self.entries.len();
        if BOT_POOL.set(self).is_err() {
            log::warn!("[BOT_POOL] install ignored — pool already installed");
        } else {
            log::info!("[BOT_POOL] installed with {} identities", n);
        }
    }

    /// Process-wide accessor. Returns `None` until `install` has run.
    pub fn get() -> Option<&'static BotPool> {
        BOT_POOL.get()
    }

    /// Pull up to `count` identities with **unique display names** within the
    /// returned batch (so a single lobby never shows two bots with the same
    /// name). Uses a partial Fisher–Yates shuffle over index space — O(count)
    /// swaps, no full sort.
    ///
    /// NOTE: this shuffle is only about NAME UNIQUENESS inside one lobby. It
    /// is NOT an entry order — the drip below consumes the batch in whatever
    /// order it lands, and the TIMING is what the observer sees. Do not
    /// "sort" or "pace" this batch. (MENTAL MODEL: organic.)
    pub fn take(&self, count: usize, rng: &mut impl Rng) -> Vec<BotPoolEntry> {
        if self.entries.is_empty() || count == 0 {
            return Vec::new();
        }
        let mut idx: Vec<usize> = (0..self.entries.len()).collect();
        let swaps = count.min(idx.len());
        for i in 0..swaps {
            let j = rng.gen_range(i..idx.len());
            idx.swap(i, j);
        }
        let mut seen_names: std::collections::HashSet<&str> = std::collections::HashSet::new();
        let mut out: Vec<BotPoolEntry> = Vec::with_capacity(count);
        for &i in &idx {
            let e = &self.entries[i];
            // The persistent account pool can be larger than the curated name
            // list and therefore reuse names. Keep each lobby visually unique.
            if !seen_names.insert(e.display_name.as_str()) {
                continue;
            }
            out.push(e.clone());
            if out.len() >= count {
                break;
            }
        }
        out
    }
}

// ── Independent-moment placement ──────────────────────────────────────────

/// Per-tick injection of fictional humans. Called from `promote_countdown`
/// every tick (10 Hz).
///
/// MENTAL MODEL (organic — read the module header before changing this):
///  1. First sighting of a lobby: roll a random fill percentage
///     (`SOW_BOT_FILL_MIN`..`SOW_BOT_FILL_MAX` of max_players). Every lobby
///     gets its own count — no two lobbies ever fill alike.
///  2. Each reserved ghost INDEPENDENTLY picks a random moment
///     (`join_at_elapsed`, uniform over the whole countdown window) to show
///     up. There is NO mean, NO drip clock, NO "next entry depends on the
///     previous one". Timing is a set of independent points on a line, not a
///     sequence. That is what kills any perceptible rate.
///  3. Leave room for external players: target is a TOTAL (external players
///     + bots). External players present → fewer bots needed → trim the pending queue. Already-met
///       ghosts stay; the lobby keeps the ghosts that were going to show up
///       soonest.
///  4. Each tick: every pending ghost whose `join_at_elapsed` has been
///     reached enters NOW. Several can land on the same tick (a burst); none
///     can land for many ticks (a silence). Both are correct and intended —
///     that is literally independent placement.
///  5. When the reserved batch runs out, the lobby stops filling. A lobby can
///     legitimately sit half-full forever. That is correct.
pub fn inject_internal_bots(games: &mut [ServerLobby]) {
    let min_pct = env::var("SOW_BOT_FILL_MIN")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .map(|v| v.clamp(0.0, 1.0))
        .unwrap_or(0.65);
    let max_pct = env::var("SOW_BOT_FILL_MAX")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .map(|v| v.clamp(min_pct, 1.0))
        .unwrap_or(0.92);

    for g in games.iter_mut() {
        if g.kind != LobbyKind::Matchmaking {
            continue;
        }
        if g.phase != LobbyPhase::CountingDown {
            continue;
        }

        let max = g.config.max_players as usize;
        let external_players = g.players.iter().filter(|p| !p.is_internal_bot).count();
        let bots = g.players.iter().filter(|p| p.is_internal_bot).count();

        // ── First sighting: this lobby gets its OWN count, and every ghost
        //    independently picks when (across the whole countdown) it shows. ─
        if g.bot_fill_target.is_none() {
            let mut rng = rand::thread_rng();
            let pct = rng.gen_range(min_pct..=max_pct);
            let target = ((max as f32) * pct).round() as usize;
            let identities: Vec<BotPoolEntry> = match BotPool::get() {
                Some(pool) if !pool.is_empty() => pool.take(target, &mut rng),
                _ => {
                    log::warn!(
                        "[BOT_FILL] Lobby {}: bot pool unavailable — skipping fill",
                        g.id
                    );
                    Vec::new()
                }
            };
            let mut identities = identities;
            let rated: Vec<u32> = g
                .players
                .iter()
                .filter(|p| !p.is_internal_bot)
                .filter_map(|p| p.league_score.map(u32::from))
                .collect();
            let league_score =
                (!rated.is_empty()).then(|| (rated.iter().sum::<u32>() / rated.len() as u32) as u16);
            let (league_tier, _) = sow_data::profile::tier_for_score(league_score.unwrap_or(0));
            let anon_count = roll_anon_count(
                target.saturating_sub(external_players),
                anon_band(league_score),
                &mut rng,
            )
            .min(identities.len());
            assign_anon_names(&mut identities, anon_count, &mut rng);
            // Each ghost picks its own moment, uniform over [0, countdown].
            // Independent draws → clusters and silences emerge naturally; no
            // mean exists for a watcher to tap a tempo to.
            let mut pending: Vec<(String, String, f32)> = identities
                .into_iter()
                .map(|e| {
                    let t = rng.gen_range(0.0..=LOBBY_COUNTDOWN_SECS);
                    (e.account_id, e.display_name, t)
                })
                .collect();
            pending.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
            log::info!(
                "[BOT_FILL] Lobby {}: target {} of {} ({:.0}%), league {}, {} Anons, {} ghosts each on their own clock",
                g.id,
                target,
                max,
                pct * 100.0,
                league_tier,
                anon_count,
                pending.len()
            );
            g.bot_fill_target = Some(target);
            g.pending_bots = pending;
        }

        // ── Leave room for external players: bots yield to real players. ──
        // Target is a TOTAL (external players + bots). External players present → fewer bots
        // needed. Trim the tail of the pending queue (latest-arriving ghosts
        // first) so a human always has room to join.
        let target = g.bot_fill_target.unwrap_or(0);
        let bots_desired = target.saturating_sub(external_players);
        let bots_needed_now = bots_desired.saturating_sub(bots);
        if g.pending_bots.len() > bots_needed_now {
            g.pending_bots.truncate(bots_needed_now);
        }

        // ── Who's due? Every ghost whose independent moment has arrived. ──
        // Sorted ascending by join_at_elapsed, so partition_point + drain
        // pops exactly the due ones in O(log n) + O(due). Nothing is drawn
        // here — the moments were all decided at lobby birth.
        let elapsed = (LOBBY_COUNTDOWN_SECS - g.countdown_secs).max(0.0);
        let split = g.pending_bots.partition_point(|(_, _, t)| *t <= elapsed);
        if split == 0 {
            continue;
        }
        let due: Vec<(String, String, f32)> = g.pending_bots.drain(..split).collect();

        let mut rng = rand::thread_rng();
        let mut pushed = false;
        for (account_id, display_name) in due
            .into_iter()
            .map(|(account_id, display_name, _)| (account_id, display_name))
        {
            let leader = Leader::nth(rng.gen_range(0..Leader::count()))
                .expect("index drawn below the hero count");
            let player_id = g
                .players
                .iter()
                .map(|p| p.player_id)
                .max()
                .unwrap_or(0)
                .saturating_add(1);

            let (dummy_tx, _dummy_rx) = mpsc::channel::<Vec<u8>>(8);

            // Teams: drop the ghost into whichever team is smaller (Red on a tie),
            // mirroring join_player. HumansVsNations: ghosts are human-side → Red.
            let team = if g.game_mode == "HumansVsNations" {
                Some(Team::Red)
            } else if g.game_mode == "Teams" {
                let reds = g
                    .players
                    .iter()
                    .filter(|p| p.team == Some(Team::Red))
                    .count();
                let blues = g
                    .players
                    .iter()
                    .filter(|p| p.team == Some(Team::Blue))
                    .count();
                Some(if blues < reds { Team::Blue } else { Team::Red })
            } else {
                None
            };

            let database_account_id = if account_id.is_empty() {
                None
            } else {
                Some(account_id)
            };

            g.players.push(PlayerConnection {
                name: display_name,
                clan_tag: String::new(),
                player_id,
                tx: dummy_tx,
                download_progress: 100,
                leader,
                random_leader: false,
                skin_style: 0,
                database_account_id,
                team,
                ip: "127.0.0.1".to_string(),
                session_id: None,
                is_internal_bot: true,
                ranked_rating: None,
                ranked_search_started_at: None,
                league_score: None,
            });
            g.ready_players.insert(player_id);
            log::debug!(
                "[BOT_FILL] Lobby {}: ghost {player_id} ({}) joined at {:.1}s ({}/{} target)",
                g.id,
                g.players.last().map(|p| p.name.as_str()).unwrap_or("?"),
                elapsed,
                g.players.len(),
                g.bot_fill_target.unwrap_or(0)
            );
            pushed = true;
        }

        // One sync per tick, not per ghost: a burst (several due on the same
        // tick) lands as a single roster update so the UI shows them arriving
        // together — exactly the "varios de golpe" that is correct here.
        if pushed {
            crate::lobby::sync_host_lobby_to_members(g);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BotPoolEntry, anon_band, assign_anon_names, roll_anon_count};
    use rand::{SeedableRng, rngs::StdRng};
    use std::collections::HashSet;

    #[test]
    fn anon_share_follows_the_league_band() {
        let mut rng = StdRng::seed_from_u64(7);
        for _ in 0..100 {
            let bronze = roll_anon_count(100, anon_band(None), &mut rng);
            assert!((50..=70).contains(&bronze));
            let silver = roll_anon_count(100, anon_band(Some(1300)), &mut rng);
            assert!((20..=35).contains(&silver));
            let gold = roll_anon_count(100, anon_band(Some(1700)), &mut rng);
            assert!((5..=10).contains(&gold));
            assert_eq!(roll_anon_count(100, anon_band(Some(2000)), &mut rng), 0);
        }
        assert_eq!(roll_anon_count(0, anon_band(None), &mut rng), 0);
    }

    #[test]
    fn anon_names_are_unique_per_lobby_and_keep_pool_accounts() {
        let mut rng = StdRng::seed_from_u64(11);
        let mut identities: Vec<_> = (0..30)
            .map(|i| BotPoolEntry {
                account_id: format!("account-{i}"),
                display_name: format!("PoolName{i}"),
            })
            .collect();
        let account_ids: Vec<_> = identities
            .iter()
            .map(|entry| entry.account_id.clone())
            .collect();

        assign_anon_names(&mut identities, 12, &mut rng);

        let anon_names: Vec<_> = identities
            .iter()
            .filter(|entry| entry.display_name.starts_with("Anon"))
            .map(|entry| entry.display_name.as_str())
            .collect();
        assert_eq!(anon_names.len(), 12);
        assert_eq!(anon_names.iter().collect::<HashSet<_>>().len(), 12);
        assert!(anon_names.iter().all(|name| {
            name.len() == 8 && name[4..].bytes().all(|byte| byte.is_ascii_digit())
        }));
        assert_eq!(
            identities
                .iter()
                .map(|entry| entry.account_id.clone())
                .collect::<Vec<_>>(),
            account_ids
        );
        assert!(
            identities[12..]
                .iter()
                .all(|entry| entry.display_name.starts_with("PoolName"))
        );
    }
}
