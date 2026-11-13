use sow_core::player::AvatarIdentity;
use sow_render::text::NameplateStatusSprite;
use web_time::Instant;

pub const MAX_DEATH_NAMEPLATES: usize = 64;
pub const DEATH_NAMEPLATE_DURATION: f32 = 1.5;
pub const GOLD_REWARD_DELAY: f32 = 0.5;
pub const GOLD_REWARD_DURATION: f32 = 3.0;

#[derive(Clone, Debug)]
pub struct DeathNameplateReward {
    pub amount: String,
    pub is_assist: bool,
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

pub(crate) fn elimination_visible_to_viewer(
    players: &[sow_core::protocol::PlayerSnapshot],
    my_id: u16,
    victim_id: u16,
    killer_id: u16,
    has_local_reward: bool,
) -> bool {
    has_local_reward
        || (my_id != 0
            && (sow_core::emote::is_recipient(players, victim_id, my_id)
                || sow_core::emote::is_recipient(players, killer_id, my_id)))
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
    pub prepared_name: sow_render::text::PreparedText,
    pub prepared_reward: Option<sow_render::text::PreparedText>,
    pub text_style_key: Option<u32>,
}

impl DeathNameplateAnimation {
    pub fn enqueue(queue: &mut Vec<Self>, animation: Self) {
        if queue.len() >= MAX_DEATH_NAMEPLATES {
            let evict = queue
                .iter()
                .position(|queued| queued.reward.is_none())
                .or_else(|| animation.reward.as_ref().map(|_| 0));
            let Some(evict) = evict else {
                return;
            };
            queue.remove(evict);
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
    let reward_elapsed = elapsed - GOLD_REWARD_DELAY;
    if !reward_elapsed.is_finite() || reward_elapsed < 0.0 || reward_elapsed >= GOLD_REWARD_DURATION
    {
        return None;
    }
    Some(reward_elapsed)
}

pub fn death_icon(by_nuke: bool) -> NameplateStatusSprite {
    if by_nuke {
        NameplateStatusSprite::Radiation
    } else {
        NameplateStatusSprite::Death
    }
}

