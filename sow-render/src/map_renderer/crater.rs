use web_time::Instant;

/// Maximum concurrent expanding craters tracked by `MapRenderer`.
/// If more arrive simultaneously, the oldest crater flushes immediately so no tile
/// is ever stranded visually unrevealed.
pub const MAX_ACTIVE_CRATER_REVEALS: usize = 16;

/// Fixed-point scale for `reveal_key` (1/64 tile precision, supports radius up to ~1023 tiles).
const KEY_SCALE: f32 = 64.0;

/// Ease-out radial progression shared between the CPU tile reveal and `blast.wgsl`.
/// Explosive burst right after impact that decelerates smoothly into the crater rim.
#[inline]
pub fn crater_front_fraction(t: f32) -> f32 {
    let u = t.clamp(0.0, 1.0);
    1.0 - (1.0 - u).powf(2.2)
}

/// Recommended crater expansion duration in seconds for a given `inner_radius`.
/// Returns `0.0` (instant reveal) when `reduced_motion` is enabled.
#[inline]
pub fn crater_expand_duration(inner_radius: u32, reduced_motion: bool) -> f32 {
    if reduced_motion || inner_radius == 0 {
        0.0
    } else {
        (0.36 + inner_radius as f32 * 0.008).clamp(0.38, 0.72)
    }
}

#[inline]
fn hash_tile_01(x: u32, y: u32) -> f32 {
    let mut h = x
        .wrapping_mul(374_761_393)
        .wrapping_add(y.wrapping_mul(668_265_263))
        .wrapping_add(0x9E37_79B9);
    h ^= h >> 13;
    h = h.wrapping_mul(1_274_126_177);
    h ^= h >> 16;
    (h & 0x00FF_FFFF) as f32 * (1.0 / 16_777_215.0)
}

#[inline]
fn reveal_key_for_tile(
    cx: i32,
    cy: i32,
    x: u32,
    y: u32,
    radius: u32,
    seed_phase: f32,
) -> u16 {
    let dx = x as i32 - cx;
    let dy = y as i32 - cy;
    let dist = ((dx * dx + dy * dy) as f32).sqrt();
    let max_r = radius.max(1) as f32;
    let rim_weight = (dist / max_r).clamp(0.0, 1.0);

    // Subtle organic angular + per-tile raggedness so the hole tears outward
    // behind the boiling fire rim instead of looking like a sterile compass wipe.
    let angle = (dy as f32).atan2(dx as f32);
    let ang_wobble =
        (angle * 5.0 + seed_phase).sin() * 0.55 + (angle * 11.0 - seed_phase * 1.7).cos() * 0.35;
    let cell_jitter = (hash_tile_01(x, y) - 0.5) * 1.5;
    let jittered = (dist + (ang_wobble + cell_jitter) * rim_weight).clamp(0.0, max_r);
    (jittered * KEY_SCALE).round().clamp(0.0, u16::MAX as f32) as u16
}

struct CraterReveal {
    cx: i32,
    cy: i32,
    radius: u32,
    radius_sq: i64,
    seed_phase: f32,
    min_x: u32,
    min_y: u32,
    box_w: u32,
    box_h: u32,
    /// Pre-nuke owners inside the local `(box_w * box_h)` bounding box.
    /// `0` means either not held, already revealed, or recaptured.
    pre_nuke_owners: Vec<u16>,
    /// `(reveal_key, tile_idx)` sorted ascending once finalized.
    ring_tiles: Vec<(u16, u32)>,
    cursor: usize,
    start_time: Instant,
    duration: f32,
    accepting_dirty: bool,
}

impl CraterReveal {
    fn new(
        cx: u32,
        cy: u32,
        radius: u32,
        duration: f32,
        start_time: Instant,
        map_w: u32,
        map_h: u32,
    ) -> Option<Self> {
        if map_w == 0 || map_h == 0 || radius == 0 || duration <= 0.0 {
            return None;
        }
        let cx_i = cx as i32;
        let cy_i = cy as i32;
        let r_i = radius as i32;
        let min_x = (cx_i - r_i).max(0) as u32;
        let max_x = (cx_i + r_i).min(map_w as i32 - 1) as u32;
        let min_y = (cy_i - r_i).max(0) as u32;
        let max_y = (cy_i + r_i).min(map_h as i32 - 1) as u32;
        if min_x > max_x || min_y > max_y {
            return None;
        }
        let box_w = max_x - min_x + 1;
        let box_h = max_y - min_y + 1;
        let box_len = (box_w as usize).saturating_mul(box_h as usize);
        let seed_phase = hash_tile_01(cx, cy) * std::f32::consts::TAU;
        Some(Self {
            cx: cx_i,
            cy: cy_i,
            radius,
            radius_sq: i64::from(radius) * i64::from(radius),
            seed_phase,
            min_x,
            min_y,
            box_w,
            box_h,
            pre_nuke_owners: vec![0; box_len],
            ring_tiles: Vec::with_capacity((box_len * 4) / 5),
            cursor: 0,
            start_time,
            duration,
            accepting_dirty: true,
        })
    }

    #[inline]
    fn local_index(&self, x: u32, y: u32) -> Option<usize> {
        if x < self.min_x || y < self.min_y {
            return None;
        }
        let lx = x - self.min_x;
        let ly = y - self.min_y;
        if lx >= self.box_w || ly >= self.box_h {
            return None;
        }
        Some((ly * self.box_w + lx) as usize)
    }

    #[inline]
    fn contains_circle(&self, x: u32, y: u32) -> bool {
        let dx = i64::from(x as i32 - self.cx);
        let dy = i64::from(y as i32 - self.cy);
        dx * dx + dy * dy <= self.radius_sq
    }

    fn flush_remaining(&mut self, map_w: u32, out_revealed: &mut Vec<u32>) {
        while self.cursor < self.ring_tiles.len() {
            let tile_idx = self.ring_tiles[self.cursor].1;
            self.cursor += 1;
            let x = tile_idx % map_w;
            let y = tile_idx / map_w;
            if let Some(li) = self.local_index(x, y)
                && self.pre_nuke_owners[li] != 0
            {
                self.pre_nuke_owners[li] = 0;
                out_revealed.push(tile_idx);
            }
        }
    }
}

/// Manages active ring-staged crater reveals for `MapRenderer`.
/// Completely zero-cost when idle (`reveals.is_empty()`).
#[derive(Default)]
pub struct CraterRevealQueue {
    reveals: Vec<CraterReveal>,
    /// Tiles flushed early due to queue capacity eviction, waiting to be packed next `update`.
    flushed_tiles: Vec<u32>,
}

impl CraterRevealQueue {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn is_active(&self) -> bool {
        !self.reveals.is_empty() || !self.flushed_tiles.is_empty()
    }

    /// Queue a newly detonated nuke crater before `MapRenderer::update` applies that tick's dirty tiles.
    pub fn enqueue(
        &mut self,
        cx: u32,
        cy: u32,
        inner_radius: u32,
        duration: f32,
        now: Instant,
        map_w: u32,
        map_h: u32,
    ) {
        let Some(reveal) = CraterReveal::new(cx, cy, inner_radius, duration, now, map_w, map_h)
        else {
            return;
        };
        if self.reveals.len() >= MAX_ACTIVE_CRATER_REVEALS {
            let mut oldest = self.reveals.remove(0);
            oldest.flush_remaining(map_w, &mut self.flushed_tiles);
            // Clear those flushed tiles from any remaining overlapping reveals too.
            for &tile_idx in &self.flushed_tiles {
                let x = tile_idx % map_w;
                let y = tile_idx / map_w;
                for other in &mut self.reveals {
                    if let Some(li) = other.local_index(x, y) {
                        other.pre_nuke_owners[li] = 0;
                    }
                }
            }
        }
        self.reveals.push(reveal);
    }

    /// Discard all reveals (e.g. on full map reset).
    pub fn clear(&mut self) {
        self.reveals.clear();
        self.flushed_tiles.clear();
    }

    /// Observe one dirty tile transition (`old_owner -> new_owner`) during `MapRenderer::update`.
    /// Returns `true` if the tile's clearing is being deferred for outward ring reveal.
    #[inline]
    pub fn intercept_dirty_tile(
        &mut self,
        tile_idx: u32,
        old_owner: u16,
        new_owner: u16,
        map_w: u32,
    ) -> bool {
        if self.reveals.is_empty() || map_w == 0 {
            return false;
        }
        let x = tile_idx % map_w;
        let y = tile_idx / map_w;

        // If the tile was claimed/recaptured by any live player, clear all held pre-nuke
        // overrides immediately so the live conqueror is never masked.
        if new_owner != 0 {
            for r in &mut self.reveals {
                if let Some(li) = r.local_index(x, y) {
                    r.pre_nuke_owners[li] = 0;
                }
            }
            return false;
        }

        if old_owner == 0 {
            return false;
        }

        let mut held = false;
        for r in &mut self.reveals {
            if !r.accepting_dirty || !r.contains_circle(x, y) {
                continue;
            }
            if let Some(li) = r.local_index(x, y) {
                if r.pre_nuke_owners[li] == 0 {
                    let key = reveal_key_for_tile(r.cx, r.cy, x, y, r.radius, r.seed_phase);
                    r.ring_tiles.push((key, tile_idx));
                }
                r.pre_nuke_owners[li] = old_owner;
                held = true;
            }
        }
        held
    }

    /// Finalize newly enqueued reveals after all `dirty_tiles` for the frame were ingested.
    pub fn finalize_pending(&mut self) {
        if self.reveals.is_empty() {
            return;
        }
        for r in &mut self.reveals {
            if r.accepting_dirty {
                r.accepting_dirty = false;
                r.ring_tiles.sort_unstable_by_key(|&(key, _)| key);
            }
        }
        // Drop water/neutral-only nukes that had no owned territory tiles to reveal.
        self.reveals.retain(|r| r.cursor < r.ring_tiles.len());
    }

    /// Advance active crater fronts to `now`, writing newly uncovered tile indices into `out_revealed`.
    pub fn advance(&mut self, now: Instant, map_w: u32, out_revealed: &mut Vec<u32>) {
        out_revealed.clear();
        if !self.flushed_tiles.is_empty() {
            out_revealed.append(&mut self.flushed_tiles);
        }
        if self.reveals.is_empty() || map_w == 0 {
            return;
        }

        // Advance each reveal up to its current radial front.
        for idx in 0..self.reveals.len() {
            let (elapsed, duration, radius) = {
                let r = &self.reveals[idx];
                (
                    now.saturating_duration_since(r.start_time).as_secs_f32(),
                    r.duration,
                    r.radius,
                )
            };
            let t = if duration <= 0.0 {
                1.0
            } else {
                (elapsed / duration).clamp(0.0, 1.0)
            };
            let front_key = if t >= 1.0 {
                u16::MAX
            } else {
                let front_r = radius as f32 * crater_front_fraction(t);
                (front_r * KEY_SCALE).floor().clamp(0.0, u16::MAX as f32) as u16
            };

            let start_out = out_revealed.len();
            {
                let r = &mut self.reveals[idx];
                while r.cursor < r.ring_tiles.len() && r.ring_tiles[r.cursor].0 <= front_key {
                    let tile_idx = r.ring_tiles[r.cursor].1;
                    r.cursor += 1;
                    let x = tile_idx % map_w;
                    let y = tile_idx / map_w;
                    if let Some(li) = r.local_index(x, y)
                        && r.pre_nuke_owners[li] != 0
                    {
                        r.pre_nuke_owners[li] = 0;
                        out_revealed.push(tile_idx);
                    }
                }
            }

            // For overlapping craters, uncovering a tile in one blast clears it across all others
            // so the earlier shockwave wins seamlessly.
            if self.reveals.len() > 1 && out_revealed.len() > start_out {
                for &tile_idx in &out_revealed[start_out..] {
                    let x = tile_idx % map_w;
                    let y = tile_idx / map_w;
                    for (other_idx, other) in self.reveals.iter_mut().enumerate() {
                        if other_idx == idx {
                            continue;
                        }
                        if let Some(li) = other.local_index(x, y) {
                            other.pre_nuke_owners[li] = 0;
                        }
                    }
                }
            }
        }

        self.reveals.retain(|r| r.cursor < r.ring_tiles.len());
    }

    /// True if `tile_idx` still holds a pre-nuke owner waiting for the shockwave front.
    #[inline]
    pub fn is_tile_held(&self, tile_idx: u32, map_w: u32) -> bool {
        if self.reveals.is_empty() || map_w == 0 {
            return false;
        }
        let x = tile_idx % map_w;
        let y = tile_idx / map_w;
        for r in &self.reveals {
            if let Some(li) = r.local_index(x, y)
                && r.pre_nuke_owners[li] != 0
            {
                return true;
            }
        }
        false
    }

    /// Visual owner for `tile_idx`: authoritative owner unless `0` and held by an expanding crater.
    #[inline]
    pub fn visual_owner(&self, tile_idx: u32, auth_owners: &[u16], map_w: u32) -> u16 {
        let auth = auth_owners[tile_idx as usize];
        if auth != 0 || self.reveals.is_empty() || map_w == 0 {
            return auth;
        }
        let x = tile_idx % map_w;
        let y = tile_idx / map_w;
        for r in &self.reveals {
            if let Some(li) = r.local_index(x, y) {
                let held = r.pre_nuke_owners[li];
                if held != 0 {
                    return held;
                }
            }
        }
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn crater_reveals_outward_monotonically_and_finishes_cleanly() {
        let map_w = 40;
        let map_h = 40;
        let mut auth = vec![7u16; (map_w * map_h) as usize];
        let mut queue = CraterRevealQueue::new();
        let t0 = Instant::now();
        let cx = 20;
        let cy = 20;
        let radius = 8;
        queue.enqueue(cx, cy, radius, 0.5, t0, map_w, map_h);

        let mut wiped = Vec::new();
        for y in 0..map_h {
            for x in 0..map_w {
                let dx = x as i32 - cx as i32;
                let dy = y as i32 - cy as i32;
                if dx * dx + dy * dy <= (radius * radius) as i32 {
                    let idx = y * map_w + x;
                    let old = auth[idx as usize];
                    auth[idx as usize] = 0;
                    assert!(queue.intercept_dirty_tile(idx, old, 0, map_w));
                    wiped.push(idx);
                }
            }
        }
        queue.finalize_pending();
        assert!(queue.is_active());

        // At t = 0.0, tiles far from center still visually show old owner 7.
        let rim_idx = cy * map_w + (cx + radius - 1);
        assert_eq!(queue.visual_owner(rim_idx, &auth, map_w), 7);

        let mut revealed = Vec::new();
        let mut total_revealed = Vec::new();

        // Mid-way: inner tiles reveal first, outer tiles still held.
        queue.advance(t0 + Duration::from_millis(120), map_w, &mut revealed);
        assert!(!revealed.is_empty());
        assert!(revealed.len() < wiped.len());
        let avg_r_early: f32 = revealed
            .iter()
            .map(|&idx| {
                let dx = (idx % map_w) as f32 - cx as f32;
                let dy = (idx / map_w) as f32 - cy as f32;
                dx.hypot(dy)
            })
            .sum::<f32>()
            / revealed.len() as f32;
        total_revealed.extend_from_slice(&revealed);

        // Completion: remaining outer tiles reveal and queue returns to idle.
        queue.advance(t0 + Duration::from_millis(550), map_w, &mut revealed);
        assert!(!revealed.is_empty());
        let avg_r_late: f32 = revealed
            .iter()
            .map(|&idx| {
                let dx = (idx % map_w) as f32 - cx as f32;
                let dy = (idx / map_w) as f32 - cy as f32;
                dx.hypot(dy)
            })
            .sum::<f32>()
            / revealed.len() as f32;
        assert!(avg_r_late > avg_r_early);
        total_revealed.extend_from_slice(&revealed);

        assert_eq!(total_revealed.len(), wiped.len());
        assert!(!queue.is_active());
        for idx in wiped {
            assert_eq!(queue.visual_owner(idx, &auth, map_w), 0);
        }
    }

    #[test]
    fn recaptured_tile_clears_held_pre_nuke_owner_immediately() {
        let map_w = 20;
        let map_h = 20;
        let mut auth = vec![3u16; (map_w * map_h) as usize];
        let mut queue = CraterRevealQueue::new();
        let t0 = Instant::now();
        queue.enqueue(10, 10, 5, 0.5, t0, map_w, map_h);

        let idx = 10 * map_w + 14;
        auth[idx as usize] = 0;
        assert!(queue.intercept_dirty_tile(idx, 3, 0, map_w));
        queue.finalize_pending();
        assert_eq!(queue.visual_owner(idx, &auth, map_w), 3);

        // Player 9 conquers the tile while the crater is still expanding.
        auth[idx as usize] = 9;
        assert!(!queue.intercept_dirty_tile(idx, 0, 9, map_w));
        assert!(!queue.is_tile_held(idx, map_w));
        assert_eq!(queue.visual_owner(idx, &auth, map_w), 9);

        let mut revealed = Vec::new();
        queue.advance(t0 + Duration::from_millis(600), map_w, &mut revealed);
        assert!(!revealed.contains(&idx));
        assert_eq!(queue.visual_owner(idx, &auth, map_w), 9);
    }

    #[test]
    fn overlapping_craters_reveal_shared_tile_once_when_first_wave_hits() {
        let map_w = 32;
        let map_h = 32;
        let mut auth = vec![5u16; (map_w * map_h) as usize];
        let mut queue = CraterRevealQueue::new();
        let t0 = Instant::now();

        // Crater A centered at (10, 16), Crater B centered at (16, 16) — shared tile at (15, 16).
        queue.enqueue(10, 16, 8, 0.8, t0, map_w, map_h);
        queue.enqueue(16, 16, 8, 0.8, t0, map_w, map_h);

        let shared = 16 * map_w + 15;
        auth[shared as usize] = 0;
        assert!(queue.intercept_dirty_tile(shared, 5, 0, map_w));
        queue.finalize_pending();

        // After 150ms, Crater B's front (distance 1) has crossed `shared`, while A's (distance 5) has not.
        let mut revealed = Vec::new();
        queue.advance(t0 + Duration::from_millis(150), map_w, &mut revealed);
        assert_eq!(revealed, vec![shared]);
        assert_eq!(queue.visual_owner(shared, &auth, map_w), 0);

        // Later when A finishes, `shared` must not be emitted a second time.
        queue.advance(t0 + Duration::from_millis(900), map_w, &mut revealed);
        assert!(revealed.is_empty());
        assert!(!queue.is_active());
    }

    #[test]
    fn reduced_motion_duration_zero_skips_queueing() {
        let mut queue = CraterRevealQueue::new();
        let dur = crater_expand_duration(22, true);
        assert_eq!(dur, 0.0);
        queue.enqueue(10, 10, 22, dur, Instant::now(), 64, 64);
        assert!(!queue.is_active());
    }
}
