use web_time::Instant;

pub const MAX_DEATH_NAMEPLATES: usize = 64;
pub const DEATH_NAMEPLATE_DURATION: f32 = 0.3;

#[derive(Clone, Debug)]
pub struct DeathNameplateAnimation {
    pub name: String,
    pub color: [f32; 3],
    pub world_x: f32,
    pub world_y: f32,
    pub start_time: Instant,
    pub by_nuke: bool,
    pub drift_x: f32,
    pub flight_distance: f32,
    pub icon_scale: f32,
}

impl DeathNameplateAnimation {
    pub fn enqueue(queue: &mut Vec<Self>, animation: Self) {
        if queue.len() >= MAX_DEATH_NAMEPLATES {
            queue.remove(0);
        }
        queue.push(animation);
    }
}

pub fn death_animation(elapsed: f32) -> Option<(f32, f32, f32)> {
    if !elapsed.is_finite() || elapsed < 0.0 || elapsed >= DEATH_NAMEPLATE_DURATION {
        return None;
    }
    let t = elapsed / DEATH_NAMEPLATE_DURATION;
    let eased = t * (2.0 - t);
    let alpha = if t < 0.1 {
        t / 0.1
    } else if t > 0.6 {
        ((1.0 - t) / 0.4).clamp(0.0, 1.0)
    } else {
        1.0
    };
    Some((t, eased, alpha))
}

pub fn death_emoji(by_nuke: bool) -> &'static str {
    if by_nuke { "☢️" } else { "🕊️" }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn animation(name: impl Into<String>) -> DeathNameplateAnimation {
        DeathNameplateAnimation {
            name: name.into(),
            color: [1.0; 3],
            world_x: 0.0,
            world_y: 0.0,
            start_time: Instant::now(),
            by_nuke: false,
            drift_x: 0.0,
            flight_distance: 15.0,
            icon_scale: 1.0,
        }
    }

    #[test]
    fn animation_eases_fades_and_expires_at_300ms() {
        assert_eq!(death_animation(0.0), Some((0.0, 0.0, 0.0)));
        assert_eq!(death_animation(0.15), Some((0.5, 0.75, 1.0)));
        assert!(death_animation(0.3).is_none());
    }

    #[test]
    fn animation_uses_dove_or_nuke_emoji() {
        assert_eq!(death_emoji(false), "🕊️");
        assert_eq!(death_emoji(true), "☢️");
    }

    #[test]
    fn queue_caps_at_64_and_evicts_the_oldest() {
        let mut queue = Vec::with_capacity(MAX_DEATH_NAMEPLATES);
        for index in 0..=MAX_DEATH_NAMEPLATES {
            DeathNameplateAnimation::enqueue(&mut queue, animation(index.to_string()));
        }
        assert_eq!(queue.len(), MAX_DEATH_NAMEPLATES);
        assert_eq!(queue.first().unwrap().name, "1");
        assert_eq!(queue.last().unwrap().name, MAX_DEATH_NAMEPLATES.to_string());
        assert!(queue.capacity() >= MAX_DEATH_NAMEPLATES);
    }
}
