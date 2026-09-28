# Security Policy

## Supported Versions

We only support the latest deployed version of the code on production servers (`shadowsofwar.io` and partner platforms).

| Component | Path / Crate | Target Stack | Status |
| --------- | ------------ | ------------ | ------ |
| sow-server | `sow-server` | Rust / Tokio / Axum / `tokio-tungstenite` | :white_check_mark: |
| sow-relay | `sow-relay` + `fstack-bridge` | Rust / Tokio (management) + F-Stack/DPDK userspace TCP with rustls TLS on game ports; HMAC-protected mgmt :8080-8083 | :white_check_mark: |
| sow-database | `sow-data` (binary `sow-database`) | Rust / Axum / Valkey (Redis-RS) / JWT | :white_check_mark: |
| sow-client / sow-render | `sow-client` + `sow-render` | Rust / WASM / Blade GPU pipeline (pinned `blade-graphics` fork) | :white_check_mark: |

## Reporting a Vulnerability

If you discover a security vulnerability, please do not open a public issue. Instead, report it privately to:

- **Email:** hello@shadowsofwar.io
- **Encryption:** (Optional) If you wish to encrypt your report, please email us to coordinate a secure keyshare.
- **Response window:** We will acknowledge receipt of your report within 48 hours and provide an estimated timeline for the fix.

## Scope & Priorities

### What We Care About Most
* **Authentication Bypass / Impersonation**: Forgery of a platform token (Google Play / Poki / CrazyGames), misuse of a canonical anonymous account ID, or forging a **relay ticket** (`ReadyWithTicket` digests are mandatory and compared constant-time — any bypass is critical).
* **Database / State Manipulation**: Unauthorized command injection or arbitrary key modification in our Valkey/Redis instance (Valkey is shared between sow-database and sow-relay — relay-side write paths are in scope).
* **Store / Billing Integrity**: Exploits in the purchase-to-grant bridge (Stripe / RevenueCat webhooks, `sow-data/src/commerce.rs`) or in server-owned currency settlement (crowns, laurels, gem spends, leader/store validation).
* **Server Crash Vectors**: Resource exhaustion or panic triggers on the `sow-relay` or `sow-database` endpoints.
* **Remote Code Execution (RCE)**: Deserialization flaws or memory safety vulnerabilities during custom map loading (`sow-map`) or asset manifest resolution.

### What Is Out of Scope / Low Severity
* **Client-Side Cheat / Memory Tampering**: Since Shadows of War uses a lockstep multiplayer networking design with server-side deterministic replay verification (`sow-relay` → replay finalize), locally modified game clients desync and their replays fail verification; they will not affect the integrity of matches on other players' clients.
* **Rate Limiting**: Minor rate-limiting issues on chat or match-finding (though reports regarding major orchestrator DDoS vectors are welcome).
* **XSS / CSRF**: The frontend marketing pages (`sow-web`) are static, and authentication tokens (`x-platform-auth`, JWT) are not stored in standard session cookies.
