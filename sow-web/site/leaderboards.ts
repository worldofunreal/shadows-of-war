import {
  createLeaderboardController,
  type LeaderboardController,
} from "../shell/sow-leaderboard-view.ts";

(() => {
  const host = document.querySelector("[data-sow-leaderboards]") as HTMLElement | null;
  if (!host) return;

  const win = window as {
    SOWTooltip?: { init?: () => void };
    SOW_I18N_READY?: Promise<unknown>;
  };
  win.SOWTooltip?.init?.();

  const controller: LeaderboardController = createLeaderboardController(host, {
    mode: "site",
    syncUrl: true,
  });

  function refreshLocale() {
    controller.rerender();
  }

  window.addEventListener("sow:locale-change", refreshLocale);
  if (win.SOW_I18N_READY && typeof win.SOW_I18N_READY.then === "function") {
    win.SOW_I18N_READY.then(refreshLocale).catch(() => {});
  }
})();
