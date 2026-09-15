//! Campaign step data + objective evaluation. This is the *content* layer: the ordered
//! table of objectives the tutorial walks through. Append objectives here — the runner in
//! [`super`] interprets them generically, so growing the campaign is data, not new code.

/// What completes a step's objective. (Add variants as new mechanics need them — each must
/// map to a field that already exists on a `PlayerSnapshot`; see `objective_progress`.)
#[derive(Clone, Copy)]
pub(super) enum Trigger {
    /// Claim N tiles since spawn (cumulative; the spawn blob exceeds small absolutes).
    TilesGained(u32),
    /// Eliminate N players ("eat N tribes") — uses the human's cumulative kill count.
    TribesEaten(u32),
    /// Defeat a SPECIFIC faction (eliminated = present in the snapshot but no longer alive). The
    /// name must match a faction in the roster (`assets/campaign/boudica.json`); a test enforces it.
    DefeatedPlayer(&'static str),
}

/// Current/target progress for a step's objective. `is_defeated(name)` reports whether a named
/// faction has been eliminated (built from the live snapshot by the runner).
pub(super) fn objective_progress(
    advance: Trigger,
    gained: u32,
    kills: u32,
    is_defeated: &dyn Fn(&str) -> bool,
) -> (u32, u32) {
    match advance {
        Trigger::TilesGained(t) => (gained.min(t), t),
        Trigger::TribesEaten(t) => (kills.min(t), t),
        Trigger::DefeatedPlayer(name) => (u32::from(is_defeated(name)), 1),
    }
}

/// Every step is one objective: the modal states it, the player taps to close it, then goes
/// and completes it; completion opens the next step's modal.
pub(super) struct Step {
    /// Dialog title + objective row label.
    pub title: &'static str,
    /// Narration shown in the dialog.
    pub body: &'static str,
    /// One terse line under the objective row that says what to *do* and what the bar counts.
    /// Keep it to a single line at the panel's width (~30 chars).
    pub hint: &'static str,
    pub advance: Trigger,
}

// ponytail: inline EN strings keep scenario iteration fast; move to sow-i18n when campaign
// localization is in scope. Keep each objective's dialog and trigger together here.
// Guide: 9 trials — start with expansion (tap to claim), learn combat (drag to attack),
// unite the east, then burn Rome's cities and ambush a legion. The final modal returns
// to the main menu with 100 Crowns, where the next episode awaits.
pub(super) const CHAPTER_1: &[Step] = &[
    Step {
        title: "Rise of the Iceni",
        body: "Welcome to the campaign. Start by tapping unclaimed land beyond your border to capture territory. Each tile generates troops and gold.",
        hint: "Tap unclaimed land to expand territory",
        advance: Trigger::TilesGained(256),
    },
    Step {
        title: "Grow the Warband",
        body: "Territory scales your economy. Continue expanding outward to increase troop generation and gold income.",
        hint: "Expand to reach 1,024 tiles held",
        advance: Trigger::TilesGained(1024),
    },
    Step {
        title: "First Contact",
        body: "Enemy outposts control adjacent territory. Learn combat: select your forces and drag into enemy borders to attack.",
        hint: "Drag to attack an outpost — defeat 1 enemy",
        advance: Trigger::TribesEaten(1),
    },
    Step {
        title: "Unite the East",
        body: "Neutralize surrounding regional outposts to secure the eastern front under your banner.",
        hint: "Defeat 4 regional outposts",
        advance: Trigger::TribesEaten(4),
    },
    Step {
        title: "Siege of Camulodunum",
        body: "Siege mechanics: surround Camulodunum and cut off reinforcements to capture the settlement.",
        hint: "Capture Camulodunum to the south",
        advance: Trigger::DefeatedPlayer("Camulodunum"),
    },
    Step {
        title: "Advance on Londinium",
        body: "Londinium controls central trade. Capture the settlement to secure the river crossing.",
        hint: "Capture Londinium",
        advance: Trigger::DefeatedPlayer("Londinium"),
    },
    Step {
        title: "Capture Verulamium",
        body: "Verulamium on Watling Street. Capture the settlement to cut imperial supply lines.",
        hint: "Capture Verulamium",
        advance: Trigger::DefeatedPlayer("Verulamium"),
    },
    Step {
        title: "Ambush the Ninth",
        body: "Legio IX Hispana is advancing from the north. Intercept and defeat their forces.",
        hint: "Intercept Legio IX Hispana",
        advance: Trigger::DefeatedPlayer("Legio IX Hispana"),
    },
    // Terminal step: completing it pops the Final Battle modal and returns to the main menu.
    Step {
        title: "The Final Battle",
        body: "Tutorial complete. You have mastered expansion, combat, and siege. Defeat Suetonius Paulinus, then return to the main menu to choose the next campaign episode or multiplayer. (100 Crowns earned)",
        hint: "Defeat Paulinus, then return to menu",
        advance: Trigger::DefeatedPlayer("Legio XIV Gemina"),
    },
];

// Six Sky episode 1 — "Arrival" (682 CE). The queen lands at Naranjo with her
// father's guard to found a new dynasty. Fast early wins, then the old blood.
pub(super) const SIX_SKY_EP1: &[Step] = &[
    Step {
        title: "Landfall",
        body: "682 CE. You are Wak Chanil Ajaw, daughter of Dos Pilas, come to rule Naranjo. Tap unclaimed land beyond your border to claim it.",
        hint: "Tap unclaimed land to expand",
        advance: Trigger::TilesGained(128),
    },
    Step {
        title: "Sacred Fire",
        body: "Three days after your arrival the stelae record a burning ritual. Grow your ground — every tile feeds your troops and gold.",
        hint: "Expand to reach 512 tiles held",
        advance: Trigger::TilesGained(512),
    },
    Step {
        title: "First Blood",
        body: "Small villages hold the shore. Drag into an enemy border to attack and take your first rival.",
        hint: "Drag to attack — defeat 1 enemy",
        advance: Trigger::TribesEaten(1),
    },
    Step {
        title: "The Villages Kneel",
        body: "The lakeside villages must learn the new dynasty's name. Subdue them all.",
        hint: "Defeat 4 villages",
        advance: Trigger::TribesEaten(4),
    },
    Step {
        title: "The Old Blood",
        body: "A claimant of Naranjo's old line denies your right. The record names K'ahk' Xiiw Chan Chaahk before you — end his heir.",
        hint: "Defeat the Rival Claimant",
        advance: Trigger::DefeatedPlayer("Rival Claimant"),
    },
    Step {
        title: "Break the Breaker",
        body: "Caracol left Naranjo kingless once before. Its garrison must fall so the city believes you can protect it.",
        hint: "Defeat Caracol",
        advance: Trigger::DefeatedPlayer("Caracol"),
    },
    Step {
        title: "Tikal's Eyes",
        body: "Tikal watches through its vanguard. Blind the great city — take its forward post.",
        hint: "Defeat the Tikal Vanguard",
        advance: Trigger::DefeatedPlayer("Tikal Vanguard"),
    },
    // Terminal step: completion returns to the campaign menu, where episode 2 unlocks.
    Step {
        title: "Queen of Naranjo",
        body: "Naranjo is yours, though the scribes never grant you its holy title. Tikal itself camps nearby — break it, and your line is secure. A son is foretold.",
        hint: "Defeat Tikal",
        advance: Trigger::DefeatedPlayer("Tikal"),
    },
];

// Six Sky episode 2 — "The Regent's Fire". Your son K'ak' Tiliw, born 688, is
// five years old and king; you rule as regent. The monuments claim five cities
// burned in five years — ritual, real fire, or royal boasting, the record does
// not say. You decide what burns.
pub(super) const SIX_SKY_EP2: &[Step] = &[
    Step {
        title: "Regent",
        body: "693 CE. Your five-year-old son is king of Naranjo, and you rule in his name. The provinces smell weakness. Expand and show strength.",
        hint: "Tap unclaimed land to expand",
        advance: Trigger::TilesGained(256),
    },
    Step {
        title: "The Provinces Stir",
        body: "Rebellion ferments in the villages. Strike first — take two rivals before they unite.",
        hint: "Defeat 2 enemies",
        advance: Trigger::TribesEaten(2),
    },
    Step {
        title: "First Fire: Uaxactun",
        body: "Uaxactun rebels. Whether the burning was rite or ruin, the stelae remember fire. Burn Uaxactun's defiance out.",
        hint: "Defeat Uaxactun Rebels",
        advance: Trigger::DefeatedPlayer("Uaxactun Rebels"),
    },
    Step {
        title: "Second Fire: Yaxha",
        body: "Yaxha on the lake rises next. Force it to reaffirm its allegiance — by kneeling or by ashes.",
        hint: "Defeat Yaxha Rebels",
        advance: Trigger::DefeatedPlayer("Yaxha Rebels"),
    },
    Step {
        title: "Third Fire: Tayasal",
        body: "Tayasal tests you. A regent who cannot hold Tayasal cannot hold Naranjo.",
        hint: "Defeat Tayasal Rebels",
        advance: Trigger::DefeatedPlayer("Tayasal Rebels"),
    },
    Step {
        title: "Fourth Fire: Seibal",
        body: "Seibal's canoes carry rebellion downriver. Sink its defiance.",
        hint: "Defeat Seibal Rebels",
        advance: Trigger::DefeatedPlayer("Seibal Rebels"),
    },
    Step {
        title: "The Breaker Returns",
        body: "Caracol, the old breaker of Naranjo, backs the rebels. Settle the oldest debt of your reign.",
        hint: "Defeat Caracol",
        advance: Trigger::DefeatedPlayer("Caracol"),
    },
    Step {
        title: "Reaffirm or Burn",
        body: "Every neighbor must choose: reaffirm allegiance to the regent, or share the rebels' fate. Keep winning.",
        hint: "Defeat 8 enemies in total",
        advance: Trigger::TribesEaten(8),
    },
    // Terminal step: completion returns to the campaign menu, where episode 3 unlocks.
    Step {
        title: "The Coalition Breaks",
        body: "Tikal gathered your enemies into one coalition. Break it and no city will doubt the regency. The moon goddess watches — 726 approaches.",
        hint: "Defeat the Tikal Coalition",
        advance: Trigger::DefeatedPlayer("Tikal Coalition"),
    },
];

// Six Sky episode 3 — "Moon Goddess" (726–741 CE). You impersonate the moon
// goddess on the Maya new year of 726, wage the last wars, and die in 741 with
// your monuments standing. The final episode: sharpest enemies, lasting legacy.
pub(super) const SIX_SKY_EP3: &[Step] = &[
    Step {
        title: "The Long Count",
        body: "The years have made you legend and target both. Expand once more — Naranjo must look strong when the goddess arrives.",
        hint: "Tap unclaimed land to expand",
        advance: Trigger::TilesGained(256),
    },
    Step {
        title: "Offerings",
        body: "Gods and soldiers both demand tribute. Take two rivals to feed the altars and the guard.",
        hint: "Defeat 2 enemies",
        advance: Trigger::TribesEaten(2),
    },
    Step {
        title: "The Unknown Enemy",
        body: "K'inichil Kab — the scribes still argue over where it rules from. Stela 24 will show its captive under your feet regardless.",
        hint: "Defeat K'inichil Kab",
        advance: Trigger::DefeatedPlayer("K'inichil Kab"),
    },
    Step {
        title: "War Host",
        body: "Caracol sends a full war host, its last throw against your line. Destroy it.",
        hint: "Defeat the Caracol War Host",
        advance: Trigger::DefeatedPlayer("Caracol War Host"),
    },
    Step {
        title: "The Hill",
        body: "Xunantunich on its hill bows to no regent. Make the hill bow.",
        hint: "Defeat Xunantunich Rebels",
        advance: Trigger::DefeatedPlayer("Xunantunich Rebels"),
    },
    Step {
        title: "Moon Goddess",
        body: "February 9, 726 — the new year. You don the moon goddess regalia before the city. Hold a great domain worthy of her: grow vast.",
        hint: "Expand to reach 1,024 tiles held",
        advance: Trigger::TilesGained(1024),
    },
    Step {
        title: "Vanguard at Dusk",
        body: "Tikal's vanguard comes at dusk, hoping age has softened you. Show them the Stela 24 queen.",
        hint: "Defeat the Tikal Vanguard",
        advance: Trigger::DefeatedPlayer("Tikal Vanguard"),
    },
    Step {
        title: "Tribute Denied",
        body: "Tikal demands tribute from Naranjo one final time. Answer with war — keep defeating all who stand with them.",
        hint: "Defeat 6 enemies in total",
        advance: Trigger::TribesEaten(6),
    },
    // Terminal step: completion returns to the main menu with the saga complete.
    Step {
        title: "Legacy in Stone",
        body: "Tikal itself. Win, and your son's line holds the city until 741 and beyond — your stelae still stand today. This is the last battle of the saga.",
        hint: "Defeat Tikal",
        advance: Trigger::DefeatedPlayer("Tikal"),
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Poka-yoke: every faction a `DefeatedPlayer` objective targets must exist in the roster
    /// (`assets/campaign/boudica.json`). Rename/remove a boss in the editor and this fails the build
    /// instead of letting the campaign get stuck on an objective that can never complete.
    #[test]
    fn objective_targets_exist_in_roster() {
        let (factions, _) = crate::campaign::boudica::roster();
        let names: std::collections::HashSet<&str> =
            factions.iter().map(|f| f.name.as_str()).collect();
        for step in CHAPTER_1 {
            if let Trigger::DefeatedPlayer(target) = step.advance {
                assert!(
                    names.contains(target),
                    "objective '{}' targets '{}', which is not in the roster",
                    step.title,
                    target
                );
            }
        }
    }

    /// Same poka-yoke for the Six Sky episodes: every `DefeatedPlayer` target
    /// must exist in its episode roster.
    #[test]
    fn six_sky_objective_targets_exist_in_rosters() {
        for (ep, steps) in [
            (1u8, super::SIX_SKY_EP1),
            (2u8, super::SIX_SKY_EP2),
            (3u8, super::SIX_SKY_EP3),
        ] {
            let (factions, _) = crate::campaign::lady_six_sky::roster(ep);
            let names: std::collections::HashSet<&str> =
                factions.iter().map(|f| f.name.as_str()).collect();
            for step in steps {
                if let Trigger::DefeatedPlayer(target) = step.advance {
                    assert!(
                        names.contains(target),
                        "ep{ep} objective '{}' targets '{}', which is not in the roster",
                        step.title,
                        target
                    );
                }
            }
        }
    }
}
