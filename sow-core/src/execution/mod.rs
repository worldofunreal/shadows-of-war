use crate::engine::SowEngine;
use crate::game::GameState;
use crate::map::TerrainType;
use crate::rng::NextIntExt;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use wyrand::WyRand;

pub mod combat;
#[cfg(test)]
mod hero_ability_tests;
pub mod hero_effects;
pub mod income;
pub mod income_rates;
pub mod nukes;
pub mod sam;

/// Fraction of refunded troops lost when retreating from an attack on another player.
pub const RETREAT_PENALTY_VS_PLAYER: f64 = 0.25;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PrioritizedTile {
    pub priority: i64,   // Lower is processed first
    pub insert_seq: u32, // Deterministic BFS tie-breaker
    pub x: u32,
    pub y: u32,
}

impl Ord for PrioritizedTile {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse order so lowest priority behaves as max in BinaryHeap (Min-Heap)
        match other.priority.cmp(&self.priority) {
            Ordering::Equal => {
                // Return exact Insertion Time inverse!
                // Older items (smallest insert_seq) pop FIRST, guaranteeing deterministic BFS organicity!
                match other.insert_seq.cmp(&self.insert_seq) {
                    Ordering::Equal => {
                        // Flawless Secondary Tie-Breaker: Coordinate space is immutable and unique per tile
                        match other.y.cmp(&self.y) {
                            Ordering::Equal => other.x.cmp(&self.x),
                            ord => ord,
                        }
                    }
                    ord => ord,
                }
            }
            ord => ord,
        }
    }
}

impl PartialOrd for PrioritizedTile {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Fixed-point unit vector scale for deterministic integer directional bias.
pub const ATTACK_DIR_SCALE: i64 = 1024;
/// Bounded angular priority penalty scale: 0° => 0, 90° => +W, 180° => +2*W.
/// Bounded so BFS advancing (`tick * 4`) naturally widens the push without 1-tile DFS spikes.
pub const DIRECTIONAL_ATTACK_WEIGHT: i64 = 48;
/// Bounded proximity penalty toward the tapped tile (`3` points per tile, capped at `64`),
/// disambiguating opposite borders along the same ray without distorting forward cones.
pub const DIRECTIONAL_DIST_STEP: i64 = 3;
pub const DIRECTIONAL_DIST_MAX: i64 = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AttackFocus {
    pub origin_x: i32,
    pub origin_y: i32,
    pub dir_x: i32,
    pub dir_y: i32,
    pub target_x: i32,
    pub target_y: i32,
}

#[inline]
pub(crate) fn isqrt_u128(n: u128) -> u128 {
    if n < 2 {
        return n;
    }
    let mut x = 1u128 << ((128 - n.leading_zeros()).div_ceil(2));
    loop {
        let next = (x + n / x) >> 1;
        if next >= x {
            return x;
        }
        x = next;
    }
}

/// Deterministic integer directional + proximity penalty in `[0, 2 * DIRECTIONAL_ATTACK_WEIGHT + DIRECTIONAL_DIST_MAX]`.
#[inline]
pub fn directional_bias(focus: Option<AttackFocus>, x: u32, y: u32) -> i64 {
    let Some(focus) = focus else {
        return 0;
    };
    let vx = i64::from(x) - i64::from(focus.origin_x);
    let vy = i64::from(y) - i64::from(focus.origin_y);
    let len_sq = vx * vx + vy * vy;
    let angle_bias = if len_sq == 0 {
        0
    } else {
        let dx = i64::from(focus.dir_x);
        let dy = i64::from(focus.dir_y);
        let dir_sq = dx * dx + dy * dy;
        if dir_sq == 0 {
            0
        } else {
            let dot = vx * dx + vy * dy;
            let cos_sq_milli = ((i128::from(dot) * i128::from(dot) * 1000)
                / (i128::from(len_sq) * i128::from(dir_sq))) as i64;
            let signed_cos_sq = if dot >= 0 {
                cos_sq_milli
            } else {
                -cos_sq_milli
            };
            ((1000 - signed_cos_sq) * DIRECTIONAL_ATTACK_WEIGHT) / 1000
        }
    };

    let tx = i64::from(x) - i64::from(focus.target_x);
    let ty = i64::from(y) - i64::from(focus.target_y);
    let target_dist = isqrt_u128((tx * tx + ty * ty) as u128) as i64;
    let dist_bias = (target_dist * DIRECTIONAL_DIST_STEP).min(DIRECTIONAL_DIST_MAX);
    angle_bias + dist_bias
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct AttackExecution {
    pub id: u64,
    pub owner_id: u16,
    pub target_owner: u16,
    /// Tick when this attack first entered execution; merges keep the original tick.
    pub created_tick: u64,
    pub troops: f64,
    pub to_conquer: BinaryHeap<PrioritizedTile>,
    pub insert_seq_counter: u32,
    pub rng: WyRand,
    /// Player cancelled via HUD; next tick refunds remaining troops (with penalty vs players).
    pub retreating: bool,
    /// The troops came ashore from ships; the Landing hero ability discounts their losses.
    /// Cleared when landed troops merge with troops that did not land.
    #[serde(default)]
    pub landed: bool,
    /// Optional directional focus from the player's tapped map tile.
    #[serde(default)]
    pub focus: Option<AttackFocus>,
}

impl AttackExecution {
    /// Adds troops to this attack. Only all-landed troops keep the landing discount, so
    /// troops that did not come ashore clear it.
    pub fn absorb_troops(&mut self, troops: f64, landed: bool) {
        self.troops += troops;
        self.landed &= landed;
    }

    /// Redirects this attack toward `new_focus` by swapping the exact directional delta
    /// on already-queued frontier tiles and re-heapifying in linear time.
    pub fn retarget_focus(&mut self, new_focus: AttackFocus) {
        let old_focus = self.focus;
        if old_focus == Some(new_focus) {
            return;
        }
        let mut tiles = std::mem::take(&mut self.to_conquer).into_vec();
        for tile in &mut tiles {
            let delta = directional_bias(Some(new_focus), tile.x, tile.y)
                - directional_bias(old_focus, tile.x, tile.y);
            tile.priority = tile.priority.saturating_add(delta);
        }
        self.to_conquer = BinaryHeap::from(tiles);
        self.focus = Some(new_focus);
    }

    /// Centroid of the queued frontier tiles (the attack's leading edge).
    /// Iterates the heap's backing slice — no pop, no alloc.
    pub fn frontier_centroid(&self) -> (f32, f32) {
        let slice = self.to_conquer.iter();
        let mut sx: u64 = 0;
        let mut sy: u64 = 0;
        let mut n: u32 = 0;
        for t in slice {
            sx += t.x as u64;
            sy += t.y as u64;
            n += 1;
        }
        if n == 0 {
            return (0.0, 0.0);
        }
        (sx as f32 / n as f32, sy as f32 / n as f32)
    }

    pub fn calc_priority(&mut self, num_owned_by_me: u32, terrain: TerrainType, tick: u64) -> i64 {
        let mag_x2 = match terrain {
            TerrainType::Land => 2,
            TerrainType::Highland => 3,
            TerrainType::Mountain => 4,
            TerrainType::Water | TerrainType::Lake => 3,
        };
        let r = self.rng.next_int(0, 7) as i64;
        // Formula scaled by 4 to maintain quartiles in integer space
        (r + 10) * (4 - (num_owned_by_me as i64 * 2) + mag_x2) + (tick as i64 * 4)
    }
}

#[inline]
pub fn fractional_extra_tiles_milli(max_tiles_f64: f64, roll_milli: u32) -> u32 {
    let frac = max_tiles_f64.fract().clamp(0.0, 0.999_999_999_999);
    let threshold_milli = (frac * 1000.0).floor() as u32; // 0..=999
    if threshold_milli > 0 && roll_milli < threshold_milli {
        1
    } else {
        0
    }
}

impl SowEngine {
}

pub fn refund_fleet_troops_to_player(game: &mut GameState, owner_id: u16, troops: f64) {
    if let Some(p) = game.player_mut(owner_id) {
        p.credit_troops_capped(troops, p.max_troops);
    }
}

#[cfg(test)]
mod troop_return_tests {
    use super::refund_fleet_troops_to_player;
    use crate::game::GameState;
    use crate::game_config::GameConfig;
    use crate::player::Player;

    #[test]
    fn troop_refunds_count_only_the_amount_that_fits_and_spending_keeps_the_total() {
        let config = GameConfig::default();
        let mut state = GameState::new(1, 2, 2, config.clone());
        let mut player = Player::new_human(1, "Player".into(), [1.0; 3], &config);
        player.troops = 900.0;
        player.max_troops = 1_000.0;
        state.register_player(player);
        let received_before = state.player(1).unwrap().troops_received;

        refund_fleet_troops_to_player(&mut state, 1, 250.0);
        assert_eq!(state.player(1).unwrap().troops, 1_000.0);
        assert_eq!(
            state.player(1).unwrap().troops_received,
            received_before + 100.0
        );

        state.player_mut(1).unwrap().troops -= 500.0;
        assert_eq!(
            state.player(1).unwrap().troops_received,
            received_before + 100.0
        );
    }
}
