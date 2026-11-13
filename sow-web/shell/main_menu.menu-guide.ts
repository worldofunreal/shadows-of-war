import { UI_TARGETS } from "./campaign/rules.ts";
import type { TutorialAnchor } from "./campaign/tutorial-guides.ts";
import type {
    CampaignDefinition,
    CampaignMachineView,
    CampaignRecord,
    CampaignStep,
} from "./campaign/types.ts";

interface MenuGuideDependencies {
    root: HTMLElement;
    tr(key: string): string;
    asset(path: string): string;
    send(type: string, fields?: CampaignRecord): boolean;
    anchorFor?(step: CampaignStep | null, hud: CampaignRecord | null): TutorialAnchor | null;
    renderContext?(
        step: CampaignStep,
        hud: CampaignRecord | null,
        anchor: TutorialAnchor | null,
        machineView: CampaignMachineView,
    ): CampaignRecord;
}

interface MenuGuideController {
    update(state: CampaignRecord, facts?: CampaignRecord): void;
    pause(): void;
}

interface GuideDescriptor {
    guideId: string;
    entryStep: string;
    endStep: string;
    targetKey: string;
    titleKey: string;
    hintKey: string;
}

interface PendingGuideSave {
    guide_id: string;
    completed_steps: string[];
    current_step: string | null;
    status: "active" | "completed" | "skipped";
    menu_tips_disabled?: boolean;
}

const POST_MATCH_PRIORITY = [
    "post_match_missions",
    "daily_ops_unlock",
    "weekly_ranked_unlock",
];
const ALL_GUIDE_IDS = ["welcome", ...POST_MATCH_PRIORITY];

const WHISPER_SVG =
    "<svg viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'>" +
    "<path d='M12 2a7 7 0 0 0-4 12.74V17a1 1 0 0 0 1 1h6a1 1 0 0 0 1-1v-2.26A7 7 0 0 0 12 2Z'/>" +
    "<path d='M9 21h6'/>" +
    "</svg>";

function escHtml(value: unknown): string {
    return String(value == null ? "" : value)
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;")
        .replace(/'/g, "&#39;");
}

function normalizedSteps(value: unknown): string[] {
    if (!Array.isArray(value)) return [];
    const seen = new Set<string>();
    value.forEach(function (step) {
        const id = String(step || "").trim();
        if (id) seen.add(id);
    });
    return Array.from(seen).sort();
}

export function createMenuGuide(dependencies: MenuGuideDependencies): MenuGuideController {
    const { tr, asset, send } = dependencies;
    let definition: CampaignDefinition | null = null;
    let loading: Promise<CampaignDefinition | null> | null = null;
    let latestState: CampaignRecord | null = null;
    let latestFacts: CampaignRecord = {};
    let accountId: string | null = null;
    let localTerminal: Record<string, boolean> = Object.create(null);
    let pendingSaves: Record<string, PendingGuideSave> = Object.create(null);
    let retryTimer: ReturnType<typeof setTimeout> | null = null;
    let activeWhisper: GuideDescriptor | null = null;
    let sessionSuppressed = false;
    let lastMatchesPlayed: number | null = null;
    let wasInMainMenu = false;
    let whisperHost: HTMLElement | null = null;
    let highlightedTarget: Element | null = null;
    let lastRenderedKey = "";

    function resetAccountTracking(nextAccountId: string | null) {
        accountId = nextAccountId;
        localTerminal = Object.create(null);
        pendingSaves = Object.create(null);
        activeWhisper = null;
        sessionSuppressed = false;
        lastMatchesPlayed = null;
        wasInMainMenu = false;
        lastRenderedKey = "";
        if (retryTimer !== null) {
            clearTimeout(retryTimer);
            retryTimer = null;
        }
    }

    function menuRoot(): HTMLElement | null {
        return document.getElementById("sow-menu");
    }

    function clearHighlight() {
        if (highlightedTarget) {
            highlightedTarget.classList.remove("is-whisper-target");
            highlightedTarget = null;
        }
        const root = menuRoot();
        if (root) {
            root.querySelectorAll(".is-whisper-target").forEach(function (el) {
                el.classList.remove("is-whisper-target");
            });
        }
    }

    function hideWhisper() {
        clearHighlight();
        activeWhisper = null;
        lastRenderedKey = "";
        if (whisperHost) {
            whisperHost.hidden = true;
            whisperHost.innerHTML = "";
        }
    }

    function loadDefinition(): Promise<CampaignDefinition | null> {
        if (definition) return Promise.resolve(definition);
        if (loading) return loading;
        loading = fetch(asset("guides/menu.triggers.json"), { cache: "no-store" })
            .then(function (response) {
                if (!response.ok) throw new Error("Menu guides are unavailable.");
                return response.json();
            })
            .then(function (parsed) {
                if (window.SOWCampaign && typeof window.SOWCampaign.validateMenuGuides === "function") {
                    const report = window.SOWCampaign.validateMenuGuides(parsed);
                    if (report && Array.isArray(report.errors) && report.errors.length) {
                        throw new Error(report.errors[0].message);
                    }
                }
                definition = parsed as CampaignDefinition;
                if (latestState) renderState();
                return definition;
            })
            .catch(function () {
                loading = null;
                return null;
            });
        return loading;
    }

    function descriptorFor(guideId: string): GuideDescriptor | null {
        if (!definition) return null;
        const guides = (definition.menu_guides || {}) as CampaignRecord;
        const meta = guides[guideId] as CampaignRecord | undefined;
        const entryStep = meta && typeof meta.entry === "string" ? meta.entry : "";
        const stepIds = meta && Array.isArray(meta.step_ids) ? (meta.step_ids as string[]) : [];
        const endStep = stepIds.length ? String(stepIds[stepIds.length - 1] || "") : "";
        const steps = Array.isArray(definition.steps) ? (definition.steps as CampaignStep[]) : [];
        const step = steps.find(function (candidate) {
            return candidate && candidate.id === entryStep;
        });
        if (!entryStep || !step) return null;
        const guideObj = (step.guide || {}) as CampaignRecord;
        const triggerObj = (step.trigger || {}) as CampaignRecord;
        const targetKey = String(guideObj.target || triggerObj.action || "");
        return {
            guideId,
            entryStep,
            endStep: endStep || entryStep,
            targetKey,
            titleKey: String(step.title_key || ""),
            hintKey: String(step.hint_key || ""),
        };
    }

    function whisperText(key: string): string {
        if (!key) return "";
        const strings = (definition && definition.strings) as Record<string, Record<string, string>> | undefined;
        if (strings) {
            const rawLocale =
                typeof window.SOW_getLocale === "function"
                    ? String(window.SOW_getLocale() || "en")
                    : "en";
            const lower = rawLocale.toLowerCase().replace(/_/g, "-");
            const base = lower.split("-")[0];
            const candidate =
                (strings[lower] && strings[lower][key]) ||
                (strings[base] && strings[base][key]) ||
                (strings.en && strings.en[key]);
            if (typeof candidate === "string" && candidate.trim()) {
                return candidate;
            }
        }
        const translated = tr(key);
        return translated && translated !== key ? translated : "";
    }

    function flushPendingSaves() {
        const keys = Object.keys(pendingSaves);
        if (!keys.length) return;
        for (const guideId of keys) {
            const payload = pendingSaves[guideId];
            if (!payload) continue;
            if (send("save_menu_guide_progress", { ...payload })) {
                delete pendingSaves[guideId];
            }
        }
        if (Object.keys(pendingSaves).length && retryTimer === null) {
            retryTimer = setTimeout(function () {
                retryTimer = null;
                flushPendingSaves();
            }, 800);
        }
    }

    function sendGuideSave(payload: PendingGuideSave) {
        pendingSaves[payload.guide_id] = payload;
        if (payload.status === "completed" || payload.status === "skipped") {
            localTerminal[payload.guide_id] = true;
        }
        if (latestState && payload.menu_tips_disabled !== undefined) {
            latestState.menu_tips_disabled = payload.menu_tips_disabled;
        }
        flushPendingSaves();
    }

    function completeGuide(desc: GuideDescriptor) {
        const steps = [desc.entryStep, desc.endStep].filter(function (value, idx, arr) {
            return Boolean(value) && arr.indexOf(value) === idx;
        });
        sendGuideSave({
            guide_id: desc.guideId,
            completed_steps: steps,
            current_step: null,
            status: "completed",
        });
        sessionSuppressed = true;
        hideWhisper();
    }

    function dismissGuide(desc: GuideDescriptor, disableAll: boolean) {
        sendGuideSave({
            guide_id: desc.guideId,
            completed_steps: [],
            current_step: desc.entryStep,
            status: "skipped",
            ...(disableAll ? { menu_tips_disabled: true } : {}),
        });
        sessionSuppressed = true;
        hideWhisper();
        if (disableAll) {
            const tipsInput = document.querySelector<HTMLInputElement>("[data-setting='menu_tips']");
            if (tipsInput) tipsInput.checked = false;
        }
    }

    function isGuideTerminal(state: CampaignRecord, desc: GuideDescriptor): boolean {
        if (localTerminal[desc.guideId]) return true;
        const guides = (state.menu_guides || {}) as Record<string, CampaignRecord>;
        const saved = guides[desc.guideId];
        if (!saved) {
            // Welcome is eligible on first launch even if map entry hasn't materialized yet.
            return desc.guideId !== "welcome";
        }
        const status = String(saved.status || "").toLowerCase();
        if (status === "completed" || status === "skipped") return true;
        const completed = normalizedSteps(saved.completed_steps);
        return completed.includes(desc.entryStep) || completed.includes(desc.endStep);
    }

    function maybeAutoCompleteObservedOutcomes(state: CampaignRecord, facts: CampaignRecord) {
        const screen = String(facts.screen || "");
        const missionsOpen = Boolean(facts.missionsOpen);
        for (const guideId of ALL_GUIDE_IDS) {
            const desc = descriptorFor(guideId);
            if (!desc || isGuideTerminal(state, desc)) continue;
            if (desc.targetKey === "menu_heroes" && screen === "heroes") {
                completeGuide(desc);
            } else if (desc.targetKey === "menu_missions" && missionsOpen) {
                completeGuide(desc);
            }
        }
    }

    function pickEligibleGuide(state: CampaignRecord, matchesPlayed: number): GuideDescriptor | null {
        const order = matchesPlayed >= 1 ? POST_MATCH_PRIORITY : ["welcome"];
        for (const guideId of order) {
            const desc = descriptorFor(guideId);
            if (!desc) continue;
            if (!isGuideTerminal(state, desc)) return desc;
        }
        return null;
    }

    function ensureWhisperHost(menuEl: HTMLElement): HTMLElement {
        if (whisperHost && whisperHost.parentElement === menuEl) return whisperHost;
        const existing = menuEl.querySelector<HTMLElement>(".sow-menu-whisper");
        if (existing) {
            whisperHost = existing;
            return existing;
        }
        const el = document.createElement("aside");
        el.className = "sow-menu-whisper";
        el.setAttribute("aria-live", "polite");
        el.hidden = true;
        menuEl.appendChild(el);
        whisperHost = el;
        return el;
    }

    function targetSelectorFor(targetKey: string): string {
        return UI_TARGETS[targetKey] || "";
    }

    function renderState() {
        const state = latestState;
        const facts = latestFacts;
        if (!state || state.phase !== "MainMenu" || Boolean(state.menu_tips_disabled)) {
            hideWhisper();
            return;
        }
        if (window.SOW_menu_flow && typeof window.SOW_menu_flow.canGuide === "function") {
            if (!window.SOW_menu_flow.canGuide()) {
                hideWhisper();
                return;
            }
        }
        if (!definition) {
            void loadDefinition();
            hideWhisper();
            return;
        }
        maybeAutoCompleteObservedOutcomes(state, facts);
        if (
            facts.missionsOpen ||
            facts.settingsOpen ||
            facts.heroInfoOpen ||
            facts.authModalOpen ||
            facts.skinPickerOpen ||
            String(facts.screen || "") !== "battle"
        ) {
            hideWhisper();
            return;
        }
        if (sessionSuppressed) {
            hideWhisper();
            return;
        }

        const matchesPlayed = Number(facts.matchesPlayed || 0);
        const candidate = pickEligibleGuide(state, matchesPlayed);
        if (!candidate) {
            hideWhisper();
            return;
        }

        const menuEl = menuRoot();
        const selector = targetSelectorFor(candidate.targetKey);
        const targetEl = selector ? document.querySelector(selector) : null;
        if (!menuEl || !targetEl || !targetEl.getClientRects().length) {
            hideWhisper();
            return;
        }

        const title = whisperText(candidate.titleKey);
        const hint = whisperText(candidate.hintKey);
        const skipLabel = whisperText("menu.guide.skip") || "Dismiss";
        const hideAllLabel = whisperText("menu.guide.hide_all") || "Hide all tips";
        if (!title && !hint) {
            hideWhisper();
            return;
        }

        activeWhisper = candidate;
        if (highlightedTarget !== targetEl) {
            clearHighlight();
            targetEl.classList.add("is-whisper-target");
            highlightedTarget = targetEl;
        }

        const host = ensureWhisperHost(menuEl);
        const renderKey = [candidate.guideId, title, hint, skipLabel, hideAllLabel].join("\0");
        if (lastRenderedKey !== renderKey || host.hidden) {
            lastRenderedKey = renderKey;
            host.dataset.whisperGuide = candidate.guideId;
            host.innerHTML =
                "<div class='sow-menu-whisper__card'>" +
                "<div class='sow-menu-whisper__head'>" +
                "<span class='sow-menu-whisper__icon' aria-hidden='true'>" +
                WHISPER_SVG +
                "</span>" +
                "<strong class='sow-menu-whisper__title'>" +
                escHtml(title) +
                "</strong>" +
                "<button type='button' class='sow-menu-whisper__close' data-whisper-action='dismiss' aria-label='" +
                escHtml(skipLabel) +
                "' data-sow-tooltip='" +
                escHtml(skipLabel) +
                "'>×</button>" +
                "</div>" +
                "<p class='sow-menu-whisper__hint'>" +
                escHtml(hint) +
                "</p>" +
                "<div class='sow-menu-whisper__foot'>" +
                "<button type='button' class='sow-menu-whisper__mute' data-whisper-action='hide-all'>" +
                escHtml(hideAllLabel) +
                "</button>" +
                "</div>" +
                "</div>";
            host.hidden = false;
        }
    }

    document.addEventListener(
        "click",
        function (event) {
            const eventTarget = event.target instanceof Element ? event.target : null;
            if (!eventTarget) return;

            const actionBtn = eventTarget.closest<HTMLElement>("[data-whisper-action]");
            if (actionBtn && whisperHost && whisperHost.contains(actionBtn)) {
                event.preventDefault();
                event.stopPropagation();
                if (!activeWhisper) {
                    hideWhisper();
                    return;
                }
                const action = actionBtn.dataset.whisperAction;
                if (action === "hide-all") {
                    dismissGuide(activeWhisper, true);
                } else {
                    dismissGuide(activeWhisper, false);
                }
                return;
            }

            if (activeWhisper) {
                const selector = targetSelectorFor(activeWhisper.targetKey);
                const targetEl = selector ? document.querySelector(selector) : null;
                if (targetEl && (targetEl === eventTarget || targetEl.contains(eventTarget))) {
                    completeGuide(activeWhisper);
                }
            }
        },
        true,
    );

    return {
        update(state: CampaignRecord, facts?: CampaignRecord) {
            if (!state) return;
            const nextAccount = typeof state.account_id === "string" ? state.account_id : null;
            if (accountId !== nextAccount) {
                resetAccountTracking(nextAccount);
            }
            const nextPhase = String(state.phase || "");
            if (nextPhase !== "MainMenu") {
                if (wasInMainMenu) {
                    sessionSuppressed = false;
                }
                wasInMainMenu = false;
                latestState = state;
                latestFacts = facts || {};
                hideWhisper();
                return;
            }
            if (!wasInMainMenu) {
                sessionSuppressed = false;
                wasInMainMenu = true;
            }
            const nextMatchesPlayed =
                facts && facts.matchesPlayed != null
                    ? Number(facts.matchesPlayed)
                    : Number((state.profile_stats as CampaignRecord | undefined)?.matches_played || 0);
            if (lastMatchesPlayed !== null && nextMatchesPlayed !== lastMatchesPlayed) {
                sessionSuppressed = false;
            }
            lastMatchesPlayed = nextMatchesPlayed;
            latestState = state;
            latestFacts = facts || {};

            // Sync pending retries once server echoes terminal state.
            const guides = (state.menu_guides || {}) as Record<string, CampaignRecord>;
            Object.keys(pendingSaves).forEach(function (guideId) {
                const pending = pendingSaves[guideId];
                const saved = guides[guideId];
                if (!pending || !saved) return;
                const statusMatches = String(saved.status || "") === pending.status;
                const tipsMatch =
                    pending.menu_tips_disabled === undefined ||
                    Boolean(state.menu_tips_disabled) === pending.menu_tips_disabled;
                if (statusMatches && tipsMatch) {
                    delete pendingSaves[guideId];
                }
            });
            flushPendingSaves();
            renderState();
        },
        pause() {
            hideWhisper();
        },
    };
}
