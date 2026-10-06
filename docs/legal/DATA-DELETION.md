# Data deletion & moderation runbook (operator-only)

Player-facing promises: `sow-web/site/privacy/` ("Your rights and deletion")
and the in-game profile controls. This is the internal procedure that
fulfills them. Bearer-gated endpoints below are loopback-bound by design —
never expose them publicly.

## 0. Self-service first

Players no longer need email for the common cases:

- **Delete my account** lives in their own profile (double-confirm). It calls
  `POST /profile/anonymous/delete` with their ownership secret and runs the
  same erasure engine as §2. Email requests still work for edge cases
  (lost device/secret: verify via creation date + linked platform instead).
- **Report player** lives on other players' profiles (closed-reason dropdown
  + free text for Other). It calls `POST /profile/anonymous/report`, always
  activates a block, stores the report for 12 months (`sow:report:{id}`,
  index `sow:reports:index`, capped at 5000), and emails the moderation
  mailbox when configured (see §4).

## 1. Receive and verify (email path)

1. Request arrives at `hello@shadowsofwar.io` with an `account_id`
   (32 hex chars, found in-game) or a display name + approximate creation date.
2. Look up the account before touching anything:
   ```sh
   ssh ionos 'valkey-cli -h 127.0.0.1 GET "sow:player:account:<account_id>"'
   ```
   Keep the same `account_id` for step 3 verification.
   If the requester gave only a display name, resolve it via
   `GET /profiles/search` on the database host first and confirm ownership
   (creation date, linked platform, recent matches) before proceeding.

## 2. Erase

```sh
ssh ionos 'curl -s -X POST http://127.0.0.1:25585/internal/profile/delete \
  -H "Authorization: Bearer $SOW_DB_SECRET" \
  -H "Content-Type: application/json" \
  -d "{\"account_id\": \"<account_id>\"}"'
```

The internal endpoint returns the account deletion report; self-service returns
`{"deleted":true,"keys_removed":N}`. Analytics cleanup runs before the account
record is removed:

- The handler derives a different keyed pseudonym for each analytics channel.
- Matching JSONL event rows are removed, including older rows that still held
  a raw account ID.
- The pseudonymous account's first-seen and dated event, cohort, active-account
  and retention memberships are removed from Valkey.
- `PlayerDb::delete_account` removes `sow:player:account:{id}` plus every
  `sow:player:identity:{provider}:{external_id}` mapping from the account's
  linked identities.
- Removes the `PLAYERS_TABLE` row and the `PUBLIC_PROFILES_TABLE` index row
  from the redb mirror.

Non-identifying daily event totals remain only until their 90-day expiry. They
do not contain a player ID and cannot be subtracted per account after
aggregation. Other players' event rows continue to age out at 90 days.

- Aggregate match-history rows remain only where required for the competitive
  record and are no longer linked to the deleted account.

## 3. Verify and reply

```sh
ssh ionos 'valkey-cli -h 127.0.0.1 GET "sow:player:account:<account_id>"'
# expect: (nil)
ssh ionos 'curl -s -o /dev/null -w "%{http_code}\n" http://127.0.0.1:25585/profiles/<account_id>'
# expect: 404
```

Reply to the requester with what was erased and the JSONL 90-day note.
Respond within 45 days of the request at most; denials must state the reason
and offer appeal by reply.

## 4. Children under 13

Same procedure, expedited: suspend first (Terms violation — underage account),
erase second, reply to the parent/guardian. Do not ask the child for new
identity documents; verify via the parent's email context instead.

## 5. Moderation queue & email setup

Review reports oldest-first:

```sh
ssh ionos 'valkey-cli -h 127.0.0.1 ZRANGE sow:reports:index 0 19 WITHSCORES'
ssh ionos 'valkey-cli -h 127.0.0.1 GET "sow:report:<report_id>"'
```

Enforcement uses the existing lobby tools (`kick_player`, `ban_player`) or a
manual `POST /internal/profile/delete` for account-level action. Chargeback
abuse is never auto-suspended — the webhook already zeroes the gems; the
human decides the account.

To activate report emails, set these in `/usr/local/etc/sow/sow.env` on IONOS
(mode 0600, same file as `SOW_DB_SECRET`) and restart `sow_database`:

```sh
SOW_SMTP_HOST=mail.worldofunreal.com
SOW_SMTP_PORT=587
SOW_SMTP_USER=<sender mailbox>
SOW_SMTP_PASS=<sender password>
SOW_SMTP_FROM=<sender mailbox>
SOW_MODERATION_EMAIL=<moderation mailbox>
```

The mailbox address lives only in that file — never in the repo, never in a
client response. Without it, reports still store + log; `email_sent=false`
in the database log tells you the queue needs manual review.
