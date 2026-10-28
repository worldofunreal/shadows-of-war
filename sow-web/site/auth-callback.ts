// Finishes an OAuth sign-in on /auth/callback: exchanges the provider code for a
// session, stores it, and sends the player back to the page they started from.

interface OAuthState {
  returnTo?: string;
  accountId?: string;
  provider?: string;
  nonce?: string;
}

interface OAuthTransaction {
  provider?: string;
  nonce?: string;
  redirectUri?: string;
  codeVerifier?: string;
}

interface Account {
  account_id?: string | number;
  id?: string | number;
  display_name?: string;
  username?: string;
}

interface SignInReply {
  session_token: string;
  account: Account;
}

interface PendingAccountSwitch {
  data: SignInReply;
  returnTo: string | undefined;
}

const loadingEl = document.getElementById("cb-loading");
const errorEl = document.getElementById("cb-error");
const errorMsgEl = document.getElementById("cb-error-msg");
const accountSwitchEl = document.getElementById("cb-account-switch");
const accountSwitchMessageEl = document.getElementById("cb-account-switch-message");
let pendingAccountSwitch: PendingAccountSwitch | null = null;

function fail(msg: string): void {
  loadingEl?.classList.add("hidden");
  accountSwitchEl?.classList.add("hidden");
  errorEl?.classList.remove("hidden");
  if (errorMsgEl) errorMsgEl.textContent = msg;
}

function readStoredJson<T>(key: string): T | null {
  try {
    return JSON.parse(localStorage.getItem(key) || "null");
  } catch {
    return null;
  }
}

// State was written by loginWithOAuth: { returnTo, accountId, provider }.
function decodeState(raw: string): OAuthState {
  if (!raw) return {};
  try {
    let b64 = raw.replace(/-/g, "+").replace(/_/g, "/");
    while (b64.length % 4 !== 0) b64 += "=";
    const obj = JSON.parse(decodeURIComponent(escape(atob(b64))));
    if (obj && typeof obj === "object") return obj;
  } catch {
    // Not base64 JSON; try plain JSON below.
  }
  try {
    const obj = JSON.parse(raw);
    if (obj && typeof obj === "object") return obj;
  } catch {
    // Not JSON either; the state is unusable.
  }
  return {};
}

// Open-redirect guard: same-origin paths only, never absolute URLs.
function safeReturnTo(raw: string | undefined): string {
  try {
    const target = new URL(raw || "/", window.location.origin);
    if (target.origin !== window.location.origin) return "/";
    return `${target.pathname}${target.search}${target.hash}` || "/";
  } catch {
    return "/";
  }
}

function clearOAuthTransaction(): void {
  try {
    sessionStorage.removeItem("wou_oauth_transaction");
    sessionStorage.removeItem("wou_oauth_provider");
  } catch {
    // Storage can be blocked; the transaction simply stays until it expires.
  }
}

function finishSignIn(data: SignInReply, returnTo: string | undefined): void {
  const oldToken = localStorage.getItem("wou_session_token");
  const oldUser = localStorage.getItem("wou_user_data");
  try {
    localStorage.setItem("wou_session_token", data.session_token);
    localStorage.setItem("wou_user_data", JSON.stringify(data.account));
  } catch (err) {
    try {
      if (oldToken === null) localStorage.removeItem("wou_session_token");
      else localStorage.setItem("wou_session_token", oldToken);
      if (oldUser === null) localStorage.removeItem("wou_user_data");
      else localStorage.setItem("wou_user_data", oldUser);
    } catch {
      // Restoring the previous session failed too; nothing more can be done.
    }
    throw err;
  }
  clearOAuthTransaction();
  window.dispatchEvent(
    new CustomEvent("wou:auth-state-change", {
      detail: {
        authenticated: true,
        isAuthenticated: true,
        user: data.account,
        token: data.session_token,
      },
    }),
  );
  window.location.replace(safeReturnTo(returnTo));
}

document.getElementById("cb-account-switch-confirm")!.addEventListener("click", () => {
  if (!pendingAccountSwitch) return;
  const pending = pendingAccountSwitch;
  pendingAccountSwitch = null;
  try {
    finishSignIn(pending.data, pending.returnTo);
  } catch (err) {
    pendingAccountSwitch = pending;
    fail((err as Error)?.message || "Could not save the selected account.");
  }
});

document.getElementById("cb-account-switch-cancel")!.addEventListener("click", () => {
  if (!pendingAccountSwitch) return;
  const pending = pendingAccountSwitch;
  pendingAccountSwitch = null;
  clearOAuthTransaction();
  window.location.replace(safeReturnTo(pending.returnTo));
});

async function completeSignIn(): Promise<void> {
  const params = new URLSearchParams(window.location.search);
  const code = params.get("code") || "";
  const stateParam = params.get("state") || "";
  const stateObj = decodeState(stateParam);

  let provider = stateObj.provider || "";
  if (!provider) {
    try {
      provider = sessionStorage.getItem("wou_oauth_provider") || "";
    } catch {
      // Storage blocked: fall back to the provider in the URL.
    }
  }
  if (!provider) provider = params.get("provider") || "";

  if (!code) return fail("No authorization code received from provider.");
  if (!provider) return fail("Could not determine provider. Please sign in again.");

  const transaction = readOAuthTransaction();
  if (!stateParam || !transaction || transaction.provider !== provider) {
    return fail("This sign-in attempt expired. Please start again.");
  }
  if (!stateObj.nonce || stateObj.nonce !== transaction.nonce) {
    return fail("This sign-in attempt could not be verified. Please start again.");
  }
  const callbackUri = `${window.location.origin}/auth/callback`;
  if (transaction.redirectUri !== callbackUri) {
    return fail("The sign-in callback does not match this game.");
  }

  try {
    const headers: Record<string, string> = {
      "Content-Type": "application/json",
      Accept: "application/json",
    };
    const sessionToken = localStorage.getItem("wou_session_token");
    if (sessionToken) headers.Authorization = `Bearer ${sessionToken}`;
    const response = await fetch(
      `https://id.worldofunreal.com/api/v1/auth/oauth/callback/${encodeURIComponent(provider)}`,
      {
        method: "POST",
        headers,
        body: JSON.stringify({
          code,
          redirect_uri: callbackUri,
          account_id: stateObj.accountId || null,
          state: stateParam,
          code_verifier: transaction.codeVerifier || null,
          context: "shadows_of_war",
        }),
      },
    );
    const reply = (await response.json().catch(() => ({}))) as Partial<SignInReply> & { error?: string };
    if (!response.ok) throw new Error(reply.error || "OAuth authentication exchange failed.");
    if (!reply.session_token || !reply.account)
      throw new Error("Identity service returned an incomplete session.");
    const data: SignInReply = { session_token: reply.session_token, account: reply.account };
    const signedInAccountId = String(data.account.account_id || data.account.id || "");
    if (!signedInAccountId) throw new Error("Identity service returned an account without an ID.");
    let currentAccountId = String(stateObj.accountId || "");
    if (!currentAccountId) {
      const currentUser = readStoredJson<Account>("wou_user_data");
      currentAccountId = String(currentUser?.account_id || "");
    }
    if (currentAccountId && currentAccountId !== signedInAccountId) {
      const currentUser = readStoredJson<Account>("wou_user_data");
      const currentName = currentUser?.display_name || currentUser?.username || "your guest account";
      const targetName = data.account.display_name || data.account.username || "this account";
      accountSwitchMessageEl!.textContent = `${targetName} is a different account. Switching will not merge ${currentName}'s progress. Continue?`;
      pendingAccountSwitch = { data, returnTo: stateObj.returnTo };
      loadingEl!.classList.add("hidden");
      accountSwitchEl!.classList.remove("hidden");
      return;
    }
    finishSignIn(data, stateObj.returnTo);
  } catch (err) {
    fail((err as Error)?.message || "Failed to exchange authorization code.");
  }
}

function readOAuthTransaction(): OAuthTransaction | null {
  try {
    return JSON.parse(sessionStorage.getItem("wou_oauth_transaction") || "null");
  } catch {
    return null;
  }
}

completeSignIn();
