import {
  isValidAccountId,
  publicProfileUrl,
  tSafe,
} from "../shell/sow-profile-helpers.ts";
import {
  createProfileController,
  renderPlayerLookup,
  type ProfileTabId,
} from "../shell/sow-profile-view.ts";

(() => {
  const host = document.querySelector("[data-sow-player-profile]") as HTMLElement | null;
  if (!host) return;

  const win = window as {
    SOWTooltip?: { init?: () => void };
    SOW_I18N_READY?: Promise<unknown>;
  };
  win.SOWTooltip?.init?.();

  let activeMount: { destroy(): void; rerender(): void } | null = null;

  function resolveAccountIdFromLocation(): string {
    const match = window.location.pathname.match(/(?:^|\/)players\/([^/?#]+)\/?$/i);
    if (match && match[1] && match[1].toLowerCase() !== "index.html") {
      try {
        return decodeURIComponent(match[1]).trim();
      } catch {
        return match[1].trim();
      }
    }
    const paramId = new URLSearchParams(window.location.search).get("id");
    return paramId ? paramId.trim() : "";
  }

  function resolveInitialTab(): ProfileTabId {
    const raw = new URLSearchParams(window.location.search).get("tab") || "";
    if (raw === "commanders" || raw === "leaders") return "leaders";
    if (raw === "history" || raw === "achievements" || raw === "overview") return raw;
    return "overview";
  }

  function syncLanguageLinks(accountId: string) {
    const cleanId = accountId && isValidAccountId(accountId) ? accountId.trim() : "";
    const hostName = String(window.location.hostname || "");
    const isLocal = hostName === "localhost" || hostName === "127.0.0.1";
    document.querySelectorAll<HTMLAnchorElement>("[data-site-language]").forEach((anchor) => {
      const code = (anchor.dataset.siteLanguage || "en").toLowerCase();
      const prefix = code === "en" ? "/" : "/" + code + "/";
      if (!cleanId) {
        anchor.href = prefix + "players/";
      } else if (isLocal) {
        anchor.href = prefix + "players/?id=" + encodeURIComponent(cleanId);
      } else {
        anchor.href = prefix + "players/" + encodeURIComponent(cleanId);
      }
    });
  }

  function mountForCurrentUrl() {
    if (activeMount) {
      activeMount.destroy();
      activeMount = null;
    }
    const accountId = resolveAccountIdFromLocation();
    syncLanguageLinks(accountId);

    if (!accountId) {
      document.title = tSafe("site.players_title", "Player Profile — Shadows of War");
      activeMount = renderPlayerLookup(host!, {
        onSelectPlayer: (nextId) => navigateToPlayer(nextId, true),
      });
      return;
    }

    if (!isValidAccountId(accountId)) {
      document.title = tSafe("site.players_title", "Player Profile — Shadows of War");
      activeMount = renderPlayerLookup(host!, {
        errorMessage: tSafe(
          "site.players_invalid_id",
          "That player identifier looks crooked. Search for a handle instead.",
        ),
        onSelectPlayer: (nextId) => navigateToPlayer(nextId, true),
      });
      return;
    }

    activeMount = createProfileController(host!, {
      accountId,
      initialTab: resolveInitialTab(),
      onNavigateProfile: (nextId) => navigateToPlayer(nextId, true),
      onProfileLoaded: (profile) => {
        const handle = String(profile.handle || "").trim();
        if (handle) {
          document.title = handle + " — Shadows of War";
        }
      },
    });
  }

  function navigateToPlayer(nextId: string, push: boolean) {
    const targetUrl = publicProfileUrl(nextId);
    if (push && typeof window.history?.pushState === "function") {
      window.history.pushState({ accountId: nextId }, "", targetUrl);
    }
    mountForCurrentUrl();
  }

  window.addEventListener("popstate", () => {
    mountForCurrentUrl();
  });

  function refreshLocale() {
    activeMount?.rerender();
  }

  window.addEventListener("sow:locale-change", refreshLocale);
  if (win.SOW_I18N_READY && typeof win.SOW_I18N_READY.then === "function") {
    win.SOW_I18N_READY.then(refreshLocale).catch(() => {});
  }

  mountForCurrentUrl();
})();
