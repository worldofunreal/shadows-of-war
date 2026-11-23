#[path = "../src/app/ability_burst.rs"]
mod ability_burst;

use ability_burst::AbilityBurstGate;
use web_time::{Duration, Instant};

#[test]
fn ability_sounds_are_once_per_burst_and_owner() {
    let start = Instant::now();
    let mut gate = AbilityBurstGate::default();

    assert!(gate.should_play(1, start));
    assert!(gate.should_play(2, start + Duration::from_millis(50)));
    assert!(!gate.should_play(2, start + Duration::from_millis(60)));

    for tick in 1..=10_u64 {
        assert!(!gate.should_play(1, start + Duration::from_millis(tick * 100)));
    }

    assert!(!gate.should_play(1, start + Duration::from_millis(2_999)));
    assert!(gate.should_play(1, start + Duration::from_millis(4_999)));

    gate.clear();
    assert!(gate.should_play(1, start + Duration::from_millis(5_000)));
}
