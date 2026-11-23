use std::collections::HashMap;

use web_time::{Duration, Instant};

const ABILITY_BURST_GAP: Duration = Duration::from_secs(2);

#[derive(Default)]
pub(crate) struct AbilityBurstGate {
    last_event_by_owner: HashMap<u16, Instant>,
}

impl AbilityBurstGate {
    pub(crate) fn should_play(&mut self, owner_id: u16, now: Instant) -> bool {
        let starts_burst = self
            .last_event_by_owner
            .get(&owner_id)
            .is_none_or(|last| now.duration_since(*last) >= ABILITY_BURST_GAP);
        self.last_event_by_owner.insert(owner_id, now);
        starts_burst
    }

    pub(crate) fn clear(&mut self) {
        self.last_event_by_owner.clear();
    }
}
