import type { MenuGoalAdvance, MenuRewardReceipt } from "./main_menu.state.ts";

export interface MatchGoalSummary {
    matchId: string;
    createdAt: number;
    advances: MenuGoalAdvance[];
    unlockedIds: string[];
}

export interface MatchGoalAchievement {
    id: string;
    title: string;
    points: number;
    target: number;
    chainId: string | null;
}

export interface MatchGoalHighlight {
    chainId: string;
    achievement: MatchGoalAchievement;
    kind: "unlock" | "near" | "jump";
    advance: MenuGoalAdvance | null;
    progressBefore: number;
    progressAfter: number;
    previousTarget: number;
    target: number;
}

const viewedMatchByAccount = new Map<string, string>();
const toastedMatchByAccount = new Map<string, string>();

function storageKey(kind: "viewed" | "toasted", accountId: string): string {
    return "sow.match-goals." + kind + "." + accountId.replace(/[^a-zA-Z0-9-]/g, "");
}

function readMatchMarker(kind: "viewed" | "toasted", accountId: string): string {
    const memory = (kind === "viewed" ? viewedMatchByAccount : toastedMatchByAccount).get(accountId);
    if (memory) return memory;
    try {
        return window.localStorage.getItem(storageKey(kind, accountId)) || "";
    } catch {
        return "";
    }
}

function writeMatchMarker(kind: "viewed" | "toasted", accountId: string, matchId: string): void {
    (kind === "viewed" ? viewedMatchByAccount : toastedMatchByAccount).set(accountId, matchId);
    try {
        window.localStorage.setItem(storageKey(kind, accountId), matchId);
    } catch {}
}

export function hasViewedMatchGoals(accountId: string, matchId: string): boolean {
    return readMatchMarker("viewed", accountId) === matchId;
}

export function markMatchGoalsViewed(accountId: string, matchId: string): void {
    writeMatchMarker("viewed", accountId, matchId);
}

export function hasHandledMatchGoalToast(accountId: string, matchId: string): boolean {
    return readMatchMarker("toasted", accountId) === matchId;
}

export function markMatchGoalToastHandled(accountId: string, matchId: string): void {
    writeMatchMarker("toasted", accountId, matchId);
}

function validAdvance(value: MenuGoalAdvance): boolean {
    return (
        typeof value.chain_id === "string" &&
        /^[a-z][a-z0-9_:]{0,95}$/.test(value.chain_id) &&
        typeof value.achievement_id === "string" &&
        /^[a-z][a-z0-9_]{0,95}$/.test(value.achievement_id) &&
        Number.isSafeInteger(value.before) &&
        Number.isSafeInteger(value.after) &&
        Number.isSafeInteger(value.previous) &&
        Number.isSafeInteger(value.target) &&
        value.after > value.before &&
        value.previous >= 0 &&
        value.target > value.previous &&
        value.after >= value.previous
    );
}

function isMatchReceipt(receipt: MenuRewardReceipt, matchId: string): boolean {
    return (
        receipt.match_id === matchId &&
        (receipt.id === matchId || receipt.id === "verified_bonus:" + matchId) &&
        (receipt.status === "applied" || receipt.status === "presented")
    );
}

function compareMatchIds(left: string, right: string): number {
    if (/^\d+$/.test(left) && /^\d+$/.test(right)) {
        const normalizedLeft = left.replace(/^0+(?=\d)/, "");
        const normalizedRight = right.replace(/^0+(?=\d)/, "");
        return normalizedLeft.length - normalizedRight.length || normalizedLeft.localeCompare(normalizedRight);
    }
    return left.localeCompare(right);
}

/** Join the participation and verified-result receipts into one match detail. */
export function latestMatchGoalSummary(receipts: MenuRewardReceipt[]): MatchGoalSummary | null {
    const byMatch = new Map<string, MenuRewardReceipt[]>();
    for (const receipt of receipts) {
        if (!receipt || typeof receipt.match_id !== "string" || !receipt.match_id) continue;
        if (receipt.id !== receipt.match_id && receipt.id !== "verified_bonus:" + receipt.match_id) continue;
        const group = byMatch.get(receipt.match_id) || [];
        group.push(receipt);
        byMatch.set(receipt.match_id, group);
    }

    const summaries: MatchGoalSummary[] = [];
    for (const [matchId, group] of byMatch) {
        const participation = group.find((receipt) => receipt.id === matchId);
        if (
            !participation ||
            (participation.verification_status !== "verified" &&
                participation.verification_status !== "rejected")
        )
            continue;
        const ordered = group
            .filter((receipt) => isMatchReceipt(receipt, matchId))
            .sort((left, right) => (left.id === matchId ? -1 : right.id === matchId ? 1 : 0));
        const advancesByChain = new Map<string, MenuGoalAdvance>();
        const unlockedIds = new Set<string>();
        let createdAt = 0;
        for (const receipt of ordered) {
            createdAt = Math.max(createdAt, Number(receipt.created_at) || 0);
            for (const id of receipt.unlocked_achievements || []) {
                if (/^[a-z][a-z0-9_]{0,95}$/.test(id)) unlockedIds.add(id);
            }
            for (const rawAdvance of receipt.goal_advances || []) {
                if (!validAdvance(rawAdvance)) continue;
                const prior = advancesByChain.get(rawAdvance.chain_id);
                advancesByChain.set(
                    rawAdvance.chain_id,
                    prior
                        ? {
                              ...rawAdvance,
                              before: prior.before,
                              previous: rawAdvance.after >= rawAdvance.target ? rawAdvance.previous : prior.previous,
                          }
                        : { ...rawAdvance },
                );
            }
        }
        summaries.push({
            matchId,
            createdAt,
            advances: Array.from(advancesByChain.values()),
            unlockedIds: Array.from(unlockedIds).sort(),
        });
    }
    summaries.sort(
        (left, right) => right.createdAt - left.createdAt || compareMatchIds(right.matchId, left.matchId),
    );
    return summaries[0] || null;
}

export type MatchGoalIconKind =
    | "hero"
    | "laurel"
    | "cup"
    | "ring_attack"
    | "attack"
    | "defense"
    | "spawn"
    | "solo"
    | "upgrade";

export interface MatchGoalChainMeta {
    labelKey: string;
    heroSlug: string | null;
    iconKind: MatchGoalIconKind;
}

export function matchGoalChainMeta(chainId: string): MatchGoalChainMeta {
    if (chainId === "series:battles") {
        return { labelKey: "profile.ach_sub_battles", heroSlug: null, iconKind: "ring_attack" };
    }
    if (chainId === "series:victories") {
        return { labelKey: "profile.ach_sub_victories", heroSlug: null, iconKind: "cup" };
    }
    if (chainId === "series:kills" || chainId === "series:assists") {
        return { labelKey: "profile.ach_sub_slaying", heroSlug: null, iconKind: "attack" };
    }
    if (chainId === "series:players") {
        return { labelKey: "profile.ach_sub_commanders_defeated", heroSlug: null, iconKind: "attack" };
    }
    if (chainId === "series:empires" || chainId === "series:tribes") {
        return { labelKey: "profile.ach_sub_conquest", heroSlug: null, iconKind: "defense" };
    }
    if (chainId === "series:laurels") {
        return { labelKey: "profile.ach_sub_laurels", heroSlug: null, iconKind: "laurel" };
    }
    if (chainId === "series:campaign") {
        return { labelKey: "profile.ach_sub_campaign", heroSlug: null, iconKind: "spawn" };
    }
    if (chainId === "series:roster") {
        return { labelKey: "profile.ach_sub_vanguard", heroSlug: null, iconKind: "solo" };
    }
    const [kind, slug] = chainId.split(":");
    if (slug && (kind === "hero_mastery" || kind === "hero_wins" || kind === "hero_recruit")) {
        return {
            labelKey:
                kind === "hero_mastery"
                    ? "profile.ach_sub_mastery"
                    : kind === "hero_wins"
                      ? "profile.ach_sub_hero_wins"
                      : "profile.ach_sub_recruit",
            heroSlug: slug,
            iconKind: "hero",
        };
    }
    if (chainId.startsWith("achievement:laurel")) {
        return { labelKey: "profile.ach_sub_laurels", heroSlug: null, iconKind: "laurel" };
    }
    return { labelKey: "profile.achievements", heroSlug: null, iconKind: "upgrade" };
}

function chainForAchievement(achievement: MatchGoalAchievement): string {
    if (achievement.chainId) return achievement.chainId;
    const id = achievement.id;
    if (
        id === "first_command" ||
        id === "battle_hardened" ||
        id.startsWith("battles_")
    )
        return "series:battles";
    if (
        id === "first_victory" ||
        id === "victory_march" ||
        id.startsWith("victories_")
    )
        return "series:victories";
    if (id.startsWith("kills_")) return "series:kills";
    if (id.startsWith("assists_")) return "series:assists";
    if (id.startsWith("players_")) return "series:players";
    if (id.startsWith("empires_")) return "series:empires";
    if (id.startsWith("tribes_")) return "series:tribes";
    if (id === "laurel_hoard" || id.startsWith("laurels_")) return "series:laurels";
    if (id.startsWith("roster_")) return "series:roster";
    if (id.startsWith("campaign_")) return "series:campaign";
    if (id.startsWith("recruit_")) return "hero_recruit:" + id.slice("recruit_".length);
    const mastery = id.indexOf("_mastery_");
    if (mastery > 0) return "hero_mastery:" + id.slice(0, mastery);
    const wins = id.indexOf("_wins_");
    if (wins > 0) return "hero_wins:" + id.slice(0, wins);
    return "achievement:" + id;
}

function motiveForChain(chainId: string): string {
    if (chainId === "series:battles" || chainId.startsWith("hero_mastery:")) return "match_count";
    if (chainId === "series:victories" || chainId.startsWith("hero_wins:")) return "victory_count";
    if (chainId.startsWith("hero_recruit:") || chainId === "achievement:leader_path")
        return "hero_progress";
    return chainId;
}

function clampToStage(value: number, previous: number, target: number): number {
    return Math.max(0, Math.min(target - previous, value - previous));
}

/** Rank one or two distinct match motives; routine progress stays in the profile. */
export function selectMatchGoalHighlights(
    summary: MatchGoalSummary,
    catalog: Map<string, MatchGoalAchievement>,
): MatchGoalHighlight[] {
    const unlockedByChain = new Map<string, MatchGoalAchievement>();
    for (const id of summary.unlockedIds) {
        const achievement = catalog.get(id);
        if (!achievement) continue;
        const chainId = chainForAchievement(achievement);
        const current = unlockedByChain.get(chainId);
        if (
            !current ||
            achievement.points > current.points ||
            (achievement.points === current.points && achievement.id < current.id)
        ) {
            unlockedByChain.set(chainId, achievement);
        }
    }

    const candidates: MatchGoalHighlight[] = [];
    const representedChains = new Set<string>();
    for (const advance of summary.advances) {
        const achievement = catalog.get(advance.achievement_id);
        if (!achievement) continue;
        const chainId = advance.chain_id;
        representedChains.add(chainId);
        const unlocked = unlockedByChain.get(chainId);
        if (unlocked) {
            candidates.push({
                chainId,
                achievement: achievement.id === unlocked.id ? achievement : unlocked,
                kind: "unlock",
                advance,
                progressBefore: advance.before,
                progressAfter: advance.after,
                previousTarget: advance.previous,
                target: advance.target,
            });
            continue;
        }

        const stage = Math.max(1, advance.target - advance.previous);
        const before = clampToStage(advance.before, advance.previous, advance.target);
        const after = clampToStage(advance.after, advance.previous, advance.target);
        const beforeRatio = before / stage;
        const afterRatio = after / stage;
        const enteredFinalRange = beforeRatio < 0.8 && afterRatio >= 0.8;
        const oneAway = advance.after < advance.target && advance.target - advance.after <= 1;
        const meaningfulJump =
            advance.after - advance.before >= 2 && (advance.after - advance.before) / stage >= 0.25;
        const kind = enteredFinalRange || oneAway ? "near" : meaningfulJump ? "jump" : null;
        if (kind) {
            candidates.push({
                chainId,
                achievement,
                kind,
                advance,
                progressBefore: advance.before,
                progressAfter: advance.after,
                previousTarget: advance.previous,
                target: advance.target,
            });
        }
    }

    for (const [chainId, achievement] of unlockedByChain) {
        if (representedChains.has(chainId)) continue;
        candidates.push({
            chainId,
            achievement,
            kind: "unlock",
            advance: null,
            progressBefore: achievement.target,
            progressAfter: achievement.target,
            previousTarget: 0,
            target: achievement.target,
        });
    }

    const kindRank = { unlock: 0, near: 1, jump: 2 };
    candidates.sort((left, right) => {
        const priority = kindRank[left.kind] - kindRank[right.kind];
        if (priority) return priority;
        if (left.kind === "unlock" && left.achievement.points !== right.achievement.points)
            return right.achievement.points - left.achievement.points;
        const leftHero = left.chainId.startsWith("hero_") ? 1 : 0;
        const rightHero = right.chainId.startsWith("hero_") ? 1 : 0;
        if (leftHero !== rightHero) return rightHero - leftHero;
        if (left.kind !== "unlock") {
            const leftRatio = (left.progressAfter - left.previousTarget) / Math.max(1, left.target - left.previousTarget);
            const rightRatio = (right.progressAfter - right.previousTarget) / Math.max(1, right.target - right.previousTarget);
            if (leftRatio !== rightRatio) return rightRatio - leftRatio;
        }
        return left.chainId.localeCompare(right.chainId) || left.achievement.id.localeCompare(right.achievement.id);
    });

    const chosen: MatchGoalHighlight[] = [];
    const motives = new Set<string>();
    for (const candidate of candidates) {
        const motive = motiveForChain(candidate.chainId);
        if (motives.has(motive)) continue;
        motives.add(motive);
        chosen.push(candidate);
        if (chosen.length === 2) break;
    }
    return chosen;
}
