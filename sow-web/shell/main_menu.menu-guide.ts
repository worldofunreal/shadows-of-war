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
    anchorFor(step: CampaignStep, hud?: CampaignRecord | null, facts?: CampaignRecord): CampaignRecord | null;
    renderContext(
        step: CampaignStep,
        hud: CampaignRecord | null,
        anchor: CampaignRecord | null,
        machineView: CampaignMachineView,
    ): CampaignRecord;
}

interface MenuGuideController {
    update(state: CampaignRecord, facts?: CampaignRecord): void;
    pause(): void;
}

export function createMenuGuide(dependencies: MenuGuideDependencies): MenuGuideController {
    const { root, tr, asset, send, anchorFor, renderContext } = dependencies;
    const menuJourney: CampaignRecord = {
        definition: null,
        loading: null,
        accountId: null,
        machine: null,
        view: null,
        state: null,
        facts: null,
        paused: true,
        updateRevision: 0,
        lastSaveAt: 0,
        quickMatchRevision: 0,
        pendingSave: null,
        saveRetryTimer: null,
    };
    function menuGuideAuthorized() {
        return Boolean(
            window.SOW_menu_flow &&
            typeof window.SOW_menu_flow.canGuide === "function" &&
            window.SOW_menu_flow.canGuide(),
        );
    }

    function menuGuideMap(state: CampaignRecord) {
        return state && state.menu_guides && typeof state.menu_guides === "object" ? state.menu_guides : {};
    }

    function menuGuideAccount(state: CampaignRecord) {
        return String((state && state.account_id) || "");
    }

    function menuGuideLocalKey(accountId: string) {
        return "sow.menu_guide.v1." + encodeURIComponent(accountId) + ".welcome";
    }

    function readMenuGuideLocal(accountId: string) {
        try {
            const value = JSON.parse(window.localStorage.getItem(menuGuideLocalKey(accountId)) || "null");
            return value && ["active", "completed", "skipped"].includes(value.status) ? value : null;
        } catch {
            return null;
        }
    }

    function normalizeMenuGuideRecord(record: CampaignRecord | null = {}) {
        record = record || {};
        const completed = Array.isArray(record.completed_steps) ? record.completed_steps : [];
        const status = ["completed", "skipped"].includes(record.status)
            ? record.status
            : completed.includes("welcome_heroes") || completed.includes("welcome_complete")
              ? "completed"
              : "active";
        return {
            completed_steps: status === "completed" ? ["welcome_heroes", "welcome_complete"] : [],
            current_step: status === "active" ? "welcome_heroes" : null,
            status: status,
        };
    }

    function mergeMenuGuideRecord(serverRecord: CampaignRecord, localRecord: CampaignRecord | null) {
        const server = normalizeMenuGuideRecord(serverRecord),
            local = normalizeMenuGuideRecord(localRecord);
        return server.status !== "active" ? server : local.status !== "active" ? local : server;
    }

    function scheduleMenuGuideSaveRetry() {
        if (menuJourney.saveRetryTimer || !menuJourney.pendingSave) return;
        menuJourney.saveRetryTimer = window.setTimeout(function () {
            menuJourney.saveRetryTimer = null;
            if (menuJourney.pendingSave) saveMenuGuideRecord(menuJourney.pendingSave);
        }, 2000);
    }

    function saveMenuGuideRecord(record: CampaignRecord) {
        const state = menuJourney.state,
            accountId = menuGuideAccount(state);
        if (!accountId || record.status === "active") return;
        const serverRecord = menuGuideMap(state).welcome;
        if (serverRecord && ["completed", "skipped"].includes(serverRecord.status)) {
            try {
                window.localStorage.removeItem(menuGuideLocalKey(accountId));
            } catch {
                /* Keep the server result. */
            }
            menuJourney.pendingSave = null;
            if (menuJourney.saveRetryTimer) window.clearTimeout(menuJourney.saveRetryTimer);
            menuJourney.saveRetryTimer = null;
            return;
        }
        try {
            window.localStorage.setItem(menuGuideLocalKey(accountId), JSON.stringify(record));
        } catch (error) {
            console.error("[SOW MENU GUIDE] Could not cache the pending result.", error);
        }
        menuJourney.pendingSave = record;
        const now = Date.now();
        if (
            serverRecord &&
            !state.menu_guide_save_pending &&
            (!menuJourney.lastSaveAt || now - menuJourney.lastSaveAt >= 2000)
        ) {
            menuJourney.lastSaveAt = now;
            send("save_menu_guide_progress", Object.assign({ guide_id: "welcome" }, record));
        }
        scheduleMenuGuideSaveRetry();
    }

    function loadMenuGuideDefinition() {
        if (menuJourney.definition) return Promise.resolve(menuJourney.definition);
        if (menuJourney.loading) return menuJourney.loading;
        menuJourney.loading = fetch(asset("guides/menu.triggers.json"), { cache: "no-store" })
            .then(function (response) {
                if (!response.ok) throw new Error("Menu guides are unavailable.");
                return response.json();
            })
            .then(function (definition) {
                const report = window.SOWCampaign.validateMenuGuides(definition);
                if (report.errors.length) throw new Error("Invalid menu guide: " + report.errors[0].message);
                menuJourney.definition = definition;
                return definition;
            })
            .finally(function () {
                menuJourney.loading = null;
            });
        return menuJourney.loading;
    }

    function menuGuideTranslate(key: string) {
        const definition = menuJourney.definition,
            strings = (definition && definition.strings) || {};
        const locale =
            typeof window.SOW_getLocale === "function"
                ? window.SOW_getLocale()
                : document.documentElement.lang || "en";
        const value =
            (strings[locale] && strings[locale][key]) ||
            (strings[String(locale).split("-")[0]] && strings[String(locale).split("-")[0]][key]) ||
            (strings.en && strings.en[key]);
        return value || tr(key);
    }

    function ensureMenuGuideView() {
        if (menuJourney.view) return;
        if (!root.isConnected) document.body.appendChild(root);
        menuJourney.view = window.SOWCampaignView.mount(root, {
            translate: menuGuideTranslate,
            asset: asset,
            roster: null,
            guideSkipLabel: menuGuideTranslate("menu.guide.skip"),
            onDismiss: skipMenuGuide,
        });
    }

    function finishMenuGuide(status: "completed" | "skipped") {
        menuJourney.machine = null;
        pauseMenuGuide();
        saveMenuGuideRecord(normalizeMenuGuideRecord({ status: status }));
    }

    function skipMenuGuide() {
        if (menuGuideAuthorized()) finishMenuGuide("skipped");
    }

    function updateMenuGuideRender() {
        if (!menuGuideAuthorized() || !menuJourney.machine || !menuJourney.view) {
            pauseMenuGuide();
            return;
        }
        const machineView = menuJourney.machine.update(menuJourney.facts || {}, {}),
            step = machineView.step;
        if (step.type === "end") {
            finishMenuGuide("completed");
            return;
        }
        const anchor = anchorFor(step, { tutorial: {} }, menuJourney.facts || {});
        const context = renderContext(step, null, anchor, machineView);
        context.reducedMotion = Boolean(
            window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches,
        );
        context.hideObjective = true;
        context.gestureLabel = menuGuideTranslate(step.hint_key);
        menuJourney.view.render(machineView, context);
    }

    function updateMenuGuide(state: CampaignRecord, facts: CampaignRecord = {}) {
        state = state || {};
        const accountId = menuGuideAccount(state);
        if (menuJourney.accountId !== accountId) {
            pauseMenuGuide();
            menuJourney.machine = null;
            menuJourney.pendingSave = null;
            menuJourney.lastSaveAt = 0;
            menuJourney.quickMatchRevision = Number(state.quick_match_revision) || 0;
            if (menuJourney.saveRetryTimer) window.clearTimeout(menuJourney.saveRetryTimer);
            menuJourney.saveRetryTimer = null;
        }
        menuJourney.accountId = accountId;
        menuJourney.state = state;
        menuJourney.facts = facts || {};
        const serverRecord = menuGuideMap(state).welcome;
        if (!accountId || !serverRecord) {
            pauseMenuGuide();
            return;
        }
        let record = mergeMenuGuideRecord(serverRecord, readMenuGuideLocal(accountId));
        const quickMatchRevision = Number(state.quick_match_revision) || 0;
        const startedMatch = quickMatchRevision > menuJourney.quickMatchRevision || state.phase === "Playing";
        menuJourney.quickMatchRevision = quickMatchRevision;
        if (
            record.status === "active" &&
            (startedMatch || Number(state.profile_stats && state.profile_stats.matches_played) > 0)
        ) {
            record = normalizeMenuGuideRecord({ status: "skipped" });
        }
        if (record.status !== "active") {
            menuJourney.machine = null;
            pauseMenuGuide();
            saveMenuGuideRecord(record);
            return;
        }
        if (!menuGuideAuthorized()) {
            pauseMenuGuide();
            return;
        }
        const updateRevision = ++menuJourney.updateRevision;
        loadMenuGuideDefinition()
            .then(function (definition: CampaignDefinition) {
                if (updateRevision !== menuJourney.updateRevision || !menuGuideAuthorized()) return;
                if (!menuJourney.machine) {
                    menuJourney.machine = window.SOWCampaign.create(
                        definition,
                        definition.menu_guides.welcome.entry,
                        null,
                    );
                }
                ensureMenuGuideView();
                menuJourney.paused = false;
                menuJourney.machine.setPaused(false);
                updateMenuGuideRender();
            })
            .catch(function (error: unknown) {
                if (updateRevision !== menuJourney.updateRevision) return;
                console.error("[SOW MENU GUIDE]", error);
                menuJourney.loading = null;
                pauseMenuGuide();
            });
    }

    function pauseMenuGuide() {
        menuJourney.updateRevision++;
        menuJourney.paused = true;
        if (menuJourney.machine) menuJourney.machine.setPaused(true);
        if (menuJourney.view) {
            menuJourney.view.destroy();
            menuJourney.view = null;
        }
    }
    return { update: updateMenuGuide, pause: pauseMenuGuide };
}
