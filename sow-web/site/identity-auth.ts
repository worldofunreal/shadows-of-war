// Owned-site session client. Social authorization codes are exchanged by
// the game backend; provider secrets never enter this module or a portal build.
type Account = { account_id: string; display_name?: string; linked_identities?: unknown[]; [key: string]: unknown };
type AuthResult = { status: string; session_token: string; refresh_token?: string; expires_at?: number; account: Account };
type AuthFailure = Error & { status?: number };
const keys = ["sow_session_token", "sow_refresh_token", "sow_user_data", "sow_session_expires"] as const;

export class SowAuthClient {
    private token = "";
    private refreshToken = "";
    private account: Account | null = null;
    private expiresAt = 0;
    private generation = 0;
    private pending: Promise<AuthResult> | null = null;
    private timer: ReturnType<typeof setTimeout> | null = null;
    private retryDelay = 1000;

    constructor() {
        try {
            this.token = localStorage.getItem(keys[0]) || "";
            this.refreshToken = localStorage.getItem(keys[1]) || "";
            this.account = JSON.parse(localStorage.getItem(keys[2]) || "null");
            this.expiresAt = Number(localStorage.getItem(keys[3])) || 0;
        } catch { /* Valid credentials can still recover an unreadable card. */ }
        if (this.token || this.refreshToken) this.schedule();
    }

    private state(status: string) {
        window.dispatchEvent(new CustomEvent("identity:session-state-change", { detail: { status } }));
    }

    private async request(path: string, body?: unknown, token = this.token): Promise<any> {
        const response = await fetch("/api/auth/" + path, {
            method: body === undefined ? "GET" : "POST",
            headers: { ...(body === undefined ? {} : { "Content-Type": "application/json" }), ...(token ? { Authorization: "Bearer " + token } : {}) },
            ...(body === undefined ? {} : { body: JSON.stringify(body) }),
            cache: "no-store",
            credentials: "same-origin",
            keepalive: path === "logout",
        });
        const result = response.status === 204 ? null : await response.json().catch(() => ({}));
        if (!response.ok) {
            const error: AuthFailure = new Error(result?.error || "Sign-in is temporarily unavailable.");
            error.status = response.status;
            throw error;
        }
        return result;
    }

    private current(): AuthResult {
        if (!this.token || !this.account?.account_id) {
            const error: AuthFailure = new Error("Sign in to restore this account."); error.status = 401; throw error;
        }
        return { status: "success", session_token: this.token, refresh_token: this.refreshToken, expires_at: this.expiresAt, account: this.account };
    }

    private publicCard(account: Account): string {
        return JSON.stringify({ account_id: account.account_id, display_name: account.display_name || "" });
    }

    setSession(token: string, account: Account, refreshToken?: string, expiresAt?: number): void {
        if (!token || !account?.account_id) throw new Error("Identity response is incomplete.");
        const refresh = refreshToken || (this.account?.account_id === account.account_id ? this.refreshToken : "");
        const expiry = expiresAt || Math.floor(Date.now() / 1000) + 14 * 60;
        const previous = keys.map(key => localStorage.getItem(key));
        try {
            localStorage.setItem(keys[0], token);
            if (refresh) localStorage.setItem(keys[1], refresh); else localStorage.removeItem(keys[1]);
            localStorage.setItem(keys[2], this.publicCard(account));
            localStorage.setItem(keys[3], String(expiry));
        } catch (cause) {
            keys.forEach((key, index) => { try { if (previous[index] === null) localStorage.removeItem(key); else localStorage.setItem(key, previous[index]!); } catch {} });
            throw new Error("Could not save the account on this device.", { cause });
        }
        this.generation++;
        this.token = token; this.refreshToken = refresh; this.account = account; this.expiresAt = expiry;
        this.retryDelay = 1000;
        this.schedule();
        this.state("ready");
    }

    private schedule(delay?: number) {
        if (this.timer) clearTimeout(this.timer);
        this.timer = setTimeout(() => {
            this.timer = null;
            this.ensureSession(true).catch((error: AuthFailure) => {
                if (error.status === 401 || error.status === 403 || (!this.token && !this.refreshToken)) return;
                this.schedule(this.retryDelay);
                this.retryDelay = Math.min(30_000, this.retryDelay * 2);
            });
        }, delay ?? Math.max(1000, (this.expiresAt - Math.floor(Date.now() / 1000) - 60) * 1000));
    }

    async ensureSession(force = false): Promise<AuthResult> {
        if (this.pending) return this.pending;
        if (!force && this.token && this.account?.account_id && this.expiresAt > Date.now() / 1000 + 60) {
            return this.current();
        }
        const generation = this.generation;
        this.state("refreshing");
        const operation = async () => {
            if (generation !== this.generation) {
                const error: AuthFailure = new Error("Account changed during refresh."); error.status = 409; throw error;
            }
            // Other tabs rotate the same durable credential. Read their result
            // inside the browser lock instead of replaying the previous token.
            const savedRefresh = localStorage.getItem(keys[1]) || "";
            if (savedRefresh && savedRefresh !== this.refreshToken) {
                const saved = JSON.parse(localStorage.getItem(keys[2]) || "null") as Account | null;
                const savedToken = localStorage.getItem(keys[0]);
                if (savedToken && saved?.account_id) {
                    this.setSession(savedToken, saved, savedRefresh, Number(localStorage.getItem(keys[3])) || undefined);
                    return this.current();
                }
            }
            let result: AuthResult;
            if (this.refreshToken) {
                result = await this.request("session/refresh", { refresh_token: this.refreshToken });
            } else if (this.token) {
                const account = await this.request("me");
                result = { status: "success", session_token: this.token, account, expires_at: this.expiresAt };
            } else {
                const error: AuthFailure = new Error("Sign in to restore this account."); error.status = 401; throw error;
            }
            if (generation !== this.generation || (localStorage.getItem(keys[1]) || "") !== savedRefresh) {
                // A sign-out or account switch in another tab takes priority.
                void this.request("logout", {refresh_token:result.refresh_token}, result.session_token).catch(()=>{});
                const error: AuthFailure = new Error("Account changed during refresh."); error.status = 409; throw error;
            }
            this.setSession(result.session_token, result.account, result.refresh_token, result.expires_at);
            return this.current();
        };
        const locks = typeof navigator !== "undefined" ? navigator.locks : undefined;
        const work = locks ? locks.request("sow-auth-refresh", operation) : operation();
        this.pending = work;
        try { return await work; }
        catch (error) {
            if (generation === this.generation) {
                const status = (error as AuthFailure).status;
                this.state(status === 401 || status === 403 ? "reauthentication_required" : "retrying");
            }
            throw error;
        } finally { if (this.pending === work) this.pending = null; }
    }

    async startAnonymous(): Promise<AuthResult> {
        if (this.token || this.refreshToken) return this.ensureSession();
        const generation = this.generation;
        const accountId = localStorage.getItem("sow_account_id");
        const secret = localStorage.getItem("sow_account_secret");
        const proof = accountId && secret ? { account_id: accountId, auth_secret: secret } : {};
        const result: AuthResult = await this.request("anonymous", proof, "");
        if (generation !== this.generation) throw new Error("Account changed during sign-in.");
        this.setSession(result.session_token, result.account, result.refresh_token, result.expires_at);
        return this.current();
    }

    async getMe(): Promise<Account | null> {
        await this.ensureSession();
        let generation = this.generation;
        let account: Account;
        try { account = await this.request("me"); }
        catch (error) {
            if ((error as AuthFailure).status !== 401) throw error;
            await this.ensureSession(true); generation = this.generation;
            account = await this.request("me");
        }
        if (generation !== this.generation) return null;
        this.account = account;
        localStorage.setItem(keys[2], this.publicCard(account));
        return account;
    }

    async logout(): Promise<void> {
        let token = this.token, refresh = this.refreshToken;
        try {
            const stored = JSON.parse(localStorage.getItem(keys[2]) || "null") as Account | null;
            if (stored?.account_id === this.account?.account_id) {
                token = localStorage.getItem(keys[0]) || token;
                refresh = localStorage.getItem(keys[1]) || refresh;
            }
        } catch { /* In-memory proof can still revoke an unreadable saved session. */ }
        this.generation++;
        this.token = ""; this.refreshToken = ""; this.account = null; this.expiresAt = 0;
        if (this.timer) clearTimeout(this.timer);
        this.timer = null;
        keys.forEach(key => { try { localStorage.removeItem(key); } catch {} });
        this.state("reauthentication_required");
        try { await this.request("logout", { refresh_token: refresh }, token); } catch { /* Local sign-out also works offline. */ }
    }
}

declare global { interface Window { sowAuth: SowAuthClient } }
if (typeof window !== "undefined") {
    if (!window.sowAuth) window.sowAuth = new SowAuthClient();
    if (typeof window.dispatchEvent === "function") {
        window.dispatchEvent(new CustomEvent("identity:auth-sdk-ready"));
    }
}
