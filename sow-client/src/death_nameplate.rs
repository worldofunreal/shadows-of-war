use sow_core::player::AvatarIdentity;
use web_time::Instant;

pub const MAX_DEATH_NAMEPLATES: usize = 64;
pub const DEATH_NAMEPLATE_DURATION: f32 = 1.0;

#[derive(Clone, Debug)]
pub struct DeathNameplateAnimation {
    pub world_x: f32,
    pub world_y: f32,
    pub curve_direction: f32,
    pub start_time: Instant,
    pub by_nuke: bool,
    pub name: String,
    pub color: [f32; 4],
    pub avatar: AvatarIdentity,
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
    let t = (elapsed / DEATH_NAMEPLATE_DURATION).min(1.0);
    let eased = t * t * (3.0 - 2.0 * t);
    let alpha = if elapsed < 0.03 {
        elapsed / 0.03
    } else if elapsed > DEATH_NAMEPLATE_DURATION - 0.1 {
        ((DEATH_NAMEPLATE_DURATION - elapsed) / 0.1).clamp(0.0, 1.0)
    } else {
        1.0
    };
    Some((t, eased, alpha))
}

pub fn death_emoji(by_nuke: bool, avatar: &AvatarIdentity) -> Option<&'static str> {
    if matches!(avatar, AvatarIdentity::Emblem { .. }) {
        return None;
    }
    Some(if by_nuke { "☢️" } else { "🕊️" })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn animation() -> DeathNameplateAnimation {
        DeathNameplateAnimation {
            world_x: 0.0,
            world_y: 0.0,
            curve_direction: 1.0,
            start_time: Instant::now(),
            by_nuke: false,
            name: "Test".to_string(),
            color: [1.0, 1.0, 1.0, 1.0],
            avatar: AvatarIdentity::Fallback,
        }
    }

    #[test]
    fn animation_rises_and_expires_within_one_second() {
        assert_eq!(death_animation(0.0), Some((0.0, 0.0, 0.0)));
        assert_eq!(death_animation(0.5), Some((0.5, 0.5, 1.0)));
        assert!(death_animation(0.95).unwrap().2 < 1.0);
        assert!(death_animation(1.0).is_none());
    }

    #[test]
    fn emblem_avatar_replaces_the_extra_death_emoji() {
        let avatar = AvatarIdentity::Emblem { symbol: "🐺" };
        assert_eq!(death_emoji(false, &avatar), None);
        assert_eq!(death_emoji(true, &avatar), None);
    }

    #[test]
    fn image_avatars_keep_the_death_emoji() {
        let portrait = AvatarIdentity::Portrait {
            slug: "test".to_string(),
            leader: None,
        };
        assert_eq!(death_emoji(false, &portrait), Some("🕊️"));
        assert_eq!(death_emoji(true, &portrait), Some("☢️"));
        assert_eq!(death_emoji(false, &AvatarIdentity::Fallback), Some("🕊️"));
    }

    #[test]
    fn queue_caps_at_64_and_evicts_the_oldest() {
        let mut queue = Vec::with_capacity(MAX_DEATH_NAMEPLATES);
        for _ in 0..=MAX_DEATH_NAMEPLATES {
            DeathNameplateAnimation::enqueue(&mut queue, animation());
        }
        assert_eq!(queue.len(), MAX_DEATH_NAMEPLATES);
        assert!(queue.capacity() >= MAX_DEATH_NAMEPLATES);
    }
}
