import { describe, expect, test } from "bun:test";
import {
    hasHandledMatchGoalToast,
    hasViewedMatchGoals,
    latestMatchGoalSummary,
    markMatchGoalToastHandled,
    markMatchGoalsViewed,
    matchGoalChainMeta,
    selectMatchGoalHighlights,
    type MatchGoalAchievement,
} from "./main_menu.match-goals.ts";
import type { MenuRewardReceipt } from "./main_menu.state.ts";

function receipt(overrides: Partial<MenuRewardReceipt>): MenuRewardReceipt {
    return {
        id: "match-1",
        match_id: "match-1",
        xp: 0,
        leader_xp: 0,
        crowns: 0,
        laurels: 0,
        unlocked_leaders: [],
        status: "presented",
        ...overrides,
    };
}

const achievement = (
    id: string,
    points: number,
    target: number,
    chainId: string,
): MatchGoalAchievement => ({ id, title: id, points, target, chainId });

describe("match goal progress", () => {
    test("joins both terminal receipts and keeps only the newest match", () => {
        const receipts = [
            receipt({
                id: "old-match",
                match_id: "old-match",
                created_at: 5,
                verification_status: "rejected",
                goal_advances: [
                    { chain_id: "series:battles", achievement_id: "battles_3", before: 2, after: 3, previous: 1, target: 3 },
                ],
            }),
            receipt({
                created_at: 10,
                verification_status: "verified",
                unlocked_achievements: ["first_command"],
                goal_advances: [
                    { chain_id: "series:battles", achievement_id: "first_command", before: 0, after: 1, previous: 0, target: 1 },
                ],
            }),
            receipt({
                id: "verified_bonus:match-1",
                created_at: 11,
                verification_status: "verified",
                unlocked_achievements: ["first_victory"],
                goal_advances: [
                    { chain_id: "series:victories", achievement_id: "first_victory", before: 0, after: 1, previous: 0, target: 1 },
                ],
            }),
            receipt({
                id: "9",
                match_id: "9",
                created_at: 20,
                verification_status: "rejected",
            }),
            receipt({
                id: "10",
                match_id: "10",
                created_at: 20,
                verification_status: "rejected",
            }),
        ];
        const summary = latestMatchGoalSummary(receipts);
        const joined = latestMatchGoalSummary(receipts.slice(0, 3));

        expect(summary?.matchId).toBe("10");
        expect(summary?.createdAt).toBe(20);
        expect(summary?.advances).toEqual([]);
        expect(summary?.unlockedIds).toEqual([]);
        expect(joined?.matchId).toBe("match-1");
        expect(joined?.createdAt).toBe(11);
        expect(joined?.advances.map((advance) => advance.chain_id).sort()).toEqual([
            "series:battles",
            "series:victories",
        ]);
        expect(joined?.unlockedIds).toEqual(["first_command", "first_victory"]);
    });

    test("ranks meaningful progress, deduplicates global and hero motives, and caps at two", () => {
        const summary = {
            matchId: "match-1",
            createdAt: 1,
            unlockedIds: ["battles_3", "boudica_mastery_3", "first_victory"],
            advances: [
                { chain_id: "series:battles", achievement_id: "battles_3", before: 2, after: 3, previous: 1, target: 3 },
                { chain_id: "hero_mastery:boudica", achievement_id: "boudica_mastery_3", before: 2, after: 3, previous: 0, target: 3 },
                { chain_id: "series:victories", achievement_id: "first_victory", before: 0, after: 1, previous: 0, target: 1 },
                { chain_id: "series:kills", achievement_id: "kills_10", before: 0, after: 3, previous: 0, target: 10 },
            ],
        };
        const catalog = new Map(
            [
                achievement("battles_3", 15, 3, "series:battles"),
                achievement("boudica_mastery_3", 25, 3, "hero_mastery:boudica"),
                achievement("first_victory", 25, 1, "series:victories"),
                achievement("kills_10", 20, 10, "series:kills"),
            ].map((entry) => [entry.id, entry]),
        );

        const highlights = selectMatchGoalHighlights(summary, catalog);
        expect(highlights).toHaveLength(2);
        expect(highlights.map((item) => item.achievement.id)).toEqual([
            "boudica_mastery_3",
            "first_victory",
        ]);
        expect(highlights.every((item) => item.kind === "unlock")).toBe(true);
    });

    test("leaves routine advances out of the toast", () => {
        const summary = {
            matchId: "match-1",
            createdAt: 1,
            unlockedIds: [],
            advances: [
                { chain_id: "series:kills", achievement_id: "kills_10", before: 1, after: 2, previous: 0, target: 10 },
            ],
        };
        const catalog = new Map([["kills_10", achievement("kills_10", 20, 10, "series:kills")]]);
        expect(selectMatchGoalHighlights(summary, catalog)).toEqual([]);
    });

    test("keeps aggregate commander unlocks separate from overall match counters", () => {
        const summary = {
            matchId: "match-1",
            createdAt: 1,
            unlockedIds: ["veteran_commander"],
            advances: [
                { chain_id: "series:battles", achievement_id: "battles_10", before: 8, after: 9, previous: 5, target: 10 },
            ],
        };
        const catalog = new Map([
            ["veteran_commander", achievement("veteran_commander", 50, 10, "")],
            ["battles_10", achievement("battles_10", 25, 10, "series:battles")],
        ]);

        const highlights = selectMatchGoalHighlights(summary, catalog);
        const commander = highlights.find((item) => item.achievement.id === "veteran_commander");
        expect(commander?.chainId).toBe("achievement:veteran_commander");
        expect(commander?.kind).toBe("unlock");
        expect(commander?.progressAfter).toBe(10);
    });

    test("tracks profile review and toast delivery per account and match", () => {
        markMatchGoalsViewed("account-a", "match-1");
        markMatchGoalToastHandled("account-a", "match-1");

        expect(hasViewedMatchGoals("account-a", "match-1")).toBe(true);
        expect(hasHandledMatchGoalToast("account-a", "match-1")).toBe(true);
        expect(hasViewedMatchGoals("account-a", "match-2")).toBe(false);
        expect(hasHandledMatchGoalToast("account-b", "match-1")).toBe(false);
    });

    test("resolves canonical chain labels and icon kinds for global, hero, and laurel goals", () => {
        expect(matchGoalChainMeta("series:battles")).toEqual({
            labelKey: "profile.ach_sub_battles",
            heroSlug: null,
            iconKind: "ring_attack",
        });
        expect(matchGoalChainMeta("series:victories").iconKind).toBe("cup");
        expect(matchGoalChainMeta("series:empires").iconKind).toBe("defense");
        expect(matchGoalChainMeta("series:laurels")).toEqual({
            labelKey: "profile.ach_sub_laurels",
            heroSlug: null,
            iconKind: "laurel",
        });
        expect(matchGoalChainMeta("hero_recruit:boudica")).toEqual({
            labelKey: "profile.ach_sub_recruit",
            heroSlug: "boudica",
            iconKind: "hero",
        });
        expect(matchGoalChainMeta("hero_mastery:caesar").iconKind).toBe("hero");
        expect(matchGoalChainMeta("achievement:unknown_goal").iconKind).toBe("upgrade");
    });
});
