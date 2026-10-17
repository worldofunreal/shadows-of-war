use sow_core::player::AvatarIdentity;
use sow_render::text::NameplateStatusSprite;
use web_time::Instant;

pub const MAX_DEATH_NAMEPLATES: usize = 64;
pub const DEATH_NAMEPLATE_DURATION: f32 = 1.5;
pub const GOLD_REWARD_DELAY: f32 = 1.0;
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
            prepared_name: sow_render::text::PreparedText::default(),
            prepared_reward: None,
            text_style_key: None,
        }
    }

    #[test]
    fn animation_rises_and_expires_after_one_and_a_half_seconds() {
        assert_eq!(death_animation(0.0), Some((0.0, 0.0, 0.0)));
        assert_eq!(death_animation(0.75), Some((0.5, 0.5, 1.0)));
        assert!(death_animation(1.45).unwrap().2 < 1.0);
        assert!(death_animation(1.499).is_some());
        assert!(death_animation(1.5).is_none());
    }

    #[test]
    fn gold_reward_starts_at_one_second_overlaps_death_and_expires_at_four() {
        assert!(death_animation(0.999).is_some());
        assert!(gold_reward_elapsed(0.999).is_none());
        assert!(death_animation(1.0).is_some());
        assert_eq!(gold_reward_elapsed(1.0), Some(0.0));
        assert_eq!(gold_reward_elapsed(1.5), Some(0.5));
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
    fn elimination_animation_is_limited_to_local_participants_and_allies() {
        use sow_core::player::{Civilization, Leader, PlayerType};
        use sow_core::protocol::{PlayerSnapshot, Team};

        fn player(id: u16) -> PlayerSnapshot {
            PlayerSnapshot {
                id,
                name: format!("Player {id}"),
                troops: 100.0,
                max_troops: 100.0,
                gold: 100.0,
                tile_count: 1,
                centroid_x: 0.0,
                centroid_y: 0.0,
                player_type: PlayerType::Human,
                color: [1.0; 3],
                team: None,
                has_spawned: true,
                alive: true,
                iq: 100,
                alliances: Vec::new(),
                alliance_timers: Default::default(),
                alliance_requests: Vec::new(),
                resource_requests: Vec::new(),
                disconnected: false,
                active_emote: None,
                can_express_emote: false,
                can_retaliate: false,
                traitor: false,
                civilization: Civilization::Rome,
                leader: Leader::Caesar,
                campaign_avatar: None,
                is_campaign_faction: false,
                skin_style: 0,
                kills: 0,
                deaths: 0,
                assists: 0,
                boats_in_use: 0,
                boat_capacity: 3,
                nuke_available: false,
                nuke_cooldown_ticks: 0,
            }
        }

        let mut me = player(1);
        let mut teammate = player(3);
        me.alliances.push(2);
        teammate.team = Some(Team::Red);
        me.team = Some(Team::Red);
        let players = vec![me, player(2), teammate, player(4), player(5)];

        assert!(elimination_visible_to_viewer(&players, 1, 4, 1, false));
        assert!(elimination_visible_to_viewer(&players, 1, 1, 4, false));
        assert!(elimination_visible_to_viewer(&players, 1, 2, 4, false));
        assert!(elimination_visible_to_viewer(&players, 1, 5, 2, false));
        assert!(elimination_visible_to_viewer(&players, 1, 3, 4, false));
        assert!(elimination_visible_to_viewer(&players, 1, 5, 3, false));
        assert!(elimination_visible_to_viewer(&players, 1, 4, 5, true));
        assert!(!elimination_visible_to_viewer(&players, 1, 4, 5, false));
        assert!(!elimination_visible_to_viewer(&players, 0, 4, 5, false));
    }

    #[test]
    fn death_indicator_stays_separate_from_the_avatar() {
        assert_eq!(death_icon(false), NameplateStatusSprite::Death);
        assert_eq!(death_icon(true), NameplateStatusSprite::Radiation);
    }

    #[test]
    fn queue_caps_at_sixty_four_and_evicts_the_oldest_unrewarded_entry() {
        let mut queue = Vec::with_capacity(MAX_DEATH_NAMEPLATES);
        for index in 0..=MAX_DEATH_NAMEPLATES {
            let mut entry = animation();
            entry.name = format!("Event {index}");
            if index == 0 {
                entry.reward = Some(DeathNameplateReward {
                    amount: "+50".into(),
                    is_assist: false,
                });
            }
            DeathNameplateAnimation::enqueue(&mut queue, entry);
        }
        assert_eq!(MAX_DEATH_NAMEPLATES, 64);
        assert_eq!(queue.len(), MAX_DEATH_NAMEPLATES);
        assert_eq!(queue.first().unwrap().name, "Event 0");
        assert!(queue.iter().any(|entry| entry.reward.is_some()));
        assert!(queue.capacity() >= MAX_DEATH_NAMEPLATES);
    }

    #[test]
    fn full_queue_keeps_local_rewards_ahead_of_unrewarded_entries() {
        let mut queue = Vec::with_capacity(MAX_DEATH_NAMEPLATES);
        for index in 0..MAX_DEATH_NAMEPLATES {
            let mut entry = animation();
            entry.name = format!("Event {index}");
            if index == 0 {
                entry.reward = Some(DeathNameplateReward {
                    amount: "+50".into(),
                    is_assist: false,
                });
            }
            DeathNameplateAnimation::enqueue(&mut queue, entry);
        }

        let mut rewarded = animation();
        rewarded.reward = Some(DeathNameplateReward {
            amount: "+25".into(),
            is_assist: true,
        });
        DeathNameplateAnimation::enqueue(&mut queue, rewarded);
        assert_eq!(queue.len(), MAX_DEATH_NAMEPLATES);
        assert_eq!(
            queue.iter().filter(|entry| entry.reward.is_some()).count(),
            2
        );

        let mut all_rewards = vec![animation(); MAX_DEATH_NAMEPLATES];
        for entry in &mut all_rewards {
            entry.reward = Some(DeathNameplateReward {
                amount: "+50".into(),
                is_assist: false,
            });
        }
        DeathNameplateAnimation::enqueue(&mut all_rewards, animation());
        assert_eq!(all_rewards.len(), MAX_DEATH_NAMEPLATES);
        assert!(all_rewards.iter().all(|entry| entry.reward.is_some()));

        let mut newest_reward = animation();
        newest_reward.reward = Some(DeathNameplateReward {
            amount: "+300".into(),
            is_assist: false,
        });
        DeathNameplateAnimation::enqueue(&mut all_rewards, newest_reward);
        assert_eq!(all_rewards.len(), MAX_DEATH_NAMEPLATES);
        assert!(all_rewards.iter().all(|entry| entry.reward.is_some()));
        assert_eq!(
            all_rewards.last().unwrap().reward.as_ref().unwrap().amount,
            "+300"
        );
    }
}
