use std::collections::VecDeque;

pub const SPAWN_INTENTS_PER_WINDOW: usize = 10;
const WINDOW_MS: f64 = 3_000.0;

#[derive(Default)]
pub struct SpawnIntentRateLimit {
    recent_ticks: VecDeque<u64>,
}

impl SpawnIntentRateLimit {
    pub fn admit_at_tick(&mut self, tick: u64, tick_rate_ms: f32) -> bool {
        if self.recent_ticks.back().is_some_and(|last| tick < *last) {
            self.recent_ticks.clear();
        }

        let tick_rate_ms = if tick_rate_ms.is_finite() && tick_rate_ms > 0.0 {
            f64::from(tick_rate_ms)
        } else {
            100.0
        };
        let window_ticks = (WINDOW_MS / tick_rate_ms).ceil().max(1.0) as u64;
        while self
            .recent_ticks
            .front()
            .is_some_and(|previous| tick.saturating_sub(*previous) >= window_ticks)
        {
            self.recent_ticks.pop_front();
        }

        if self.recent_ticks.len() >= SPAWN_INTENTS_PER_WINDOW {
            return false;
        }
        self.recent_ticks.push_back(tick);
        true
    }

    pub fn reset(&mut self) {
        self.recent_ticks.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::SpawnIntentRateLimit;

    #[test]
    fn allows_ten_in_a_rolling_three_second_window() {
        let mut limit = SpawnIntentRateLimit::default();
        for tick in 0..10 {
            assert!(limit.admit_at_tick(tick, 100.0));
        }
        assert!(!limit.admit_at_tick(29, 100.0));
        assert!(limit.admit_at_tick(30, 100.0));
        assert!(!limit.admit_at_tick(30, 100.0));
    }

    #[test]
    fn tick_rate_changes_window_and_new_match_tick_resets_history() {
        let mut limit = SpawnIntentRateLimit::default();
        for tick in 0..10 {
            assert!(limit.admit_at_tick(tick, 250.0));
        }
        assert!(!limit.admit_at_tick(11, 250.0));
        assert!(limit.admit_at_tick(12, 250.0));
        assert!(limit.admit_at_tick(0, 250.0));
    }
}
