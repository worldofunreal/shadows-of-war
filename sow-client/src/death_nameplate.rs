use sow_core::player::AvatarIdentity;
use web_time::Instant;

pub const MAX_DEATH_NAMEPLATES: usize = 64;
pub const DEATH_NAMEPLATE_DURATION: f32 = 1.0;
pub const GOLD_REWARD_DURATION: f32 = 3.0;

#[derive(Clone, Debug)]
pub struct DeathNameplateReward {
    pub text: String,
    pub world_x: f32,
    pub world_y: f32,
}

pub(crate) fn local_gold_reward(
    my_id: u16,
    conqueror_id: u16,
    gold_bounty: u32,
    assists: &[(u16, u32)],
) -> Option<(u32, bool)> {
    if my_id == 0 {
        return None;
    }
    if conqueror_id == my_id && gold_bounty > 0 {
        return Some((gold_bounty, false));
    }
    assists
        .iter()
        .find(|(assist_id, gold)| *assist_id == my_id && *gold > 0)
        .map(|(_, gold)| (*gold, true))
}

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
    pub reward: Option<DeathNameplateReward>,
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

pub fn gold_reward_elapsed(elapsed: f32) -> Option<f32> {
    let reward_elapsed = elapsed - DEATH_NAMEPLATE_DURATION;
    if !reward_elapsed.is_finite() || reward_elapsed < 0.0 || reward_elapsed >= GOLD_REWARD_DURATION
    {
        return None;
    }
    Some(reward_elapsed)
}

pub fn death_emoji(by_nuke: bool) -> &'static str {
    if by_nuke { "☢️" } else { "🕊️" }
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
            reward: None,
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
    fn gold_reward_starts_after_death_and_expires_after_three_seconds() {
        assert!(death_animation(0.999).is_some());
        assert!(gold_reward_elapsed(0.999).is_none());
        assert!(death_animation(1.0).is_none());
        assert_eq!(gold_reward_elapsed(1.0), Some(0.0));
        assert_eq!(gold_reward_elapsed(3.999), Some(2.999));
        assert!(gold_reward_elapsed(4.0).is_none());
    }

    #[test]
    fn gold_reward_only_targets_the_killer_or_paid_assistant() {
        assert_eq!(local_gold_reward(2, 2, 150, &[]), Some((150, false)));
        assert_eq!(local_gold_reward(3, 2, 150, &[(3, 25)]), Some((25, true)));
        assert_eq!(local_gold_reward(4, 2, 150, &[(3, 25)]), None);
        assert_eq!(local_gold_reward(2, 2, 0, &[]), None);
        assert_eq!(local_gold_reward(0, 0, 150, &[]), None);
    }

    #[test]
    fn death_indicator_stays_separate_from_the_avatar() {
        assert_eq!(death_emoji(false), "🕊️");
        assert_eq!(death_emoji(true), "☢️");
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
