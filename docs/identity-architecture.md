# Shadows of War identity contract

## One account, separate databases

`account_id` is the only account key exposed by Shadows of War. It is stable
across Web, Android, RevenueCat, and Stripe.

- **WOU-ID** owns sign-in, sessions, verified email, and provider links.
- **SOW** owns the game record: progress, leaders, currencies, inventory,
  matches, ratings, and purchase grants.
- The databases stay separate. SOW receives the canonical `account_id`; it
  does not copy WOU-ID's provider database.

Provider subjects are lookup data, not account keys. WOU-ID keeps the subject
for Google, Play Games, Apple, Discord, Facebook, CrazyGames, Poki, Steam,
Epic, and wallets, and maps it to one `account_id`.

## Email linking

Email is a verified login method, not the account key.

- OTP creates or restores the account indexed by the normalized email.
- Google and Discord may link by email only when the provider marks that email
  verified.
- Facebook, X, and other providers link by their verified provider subject;
  an unverified email never merges accounts.
- If an email and provider already point to different accounts, login stops
  with a conflict. The system never silently merges or overwrites progress.

This makes OTP, social login, and platform login converge on the same account
without treating a matching email string as proof by itself.

## Runtime contract

Every client follows the same rules:

1. Restore the stored `account_id`.
2. Use a verified platform session when one is available.
3. On production Web, create or restore the anonymous WOU-ID session before
   WASM starts, so SOW uses that same `account_id` from the first frame.
4. On Android wrapper fallback, portals, or an identity-service outage, use the
   local anonymous SOW account and keep its ownership secret.
5. Replace the active session only after verification succeeds.

An anonymous SOW fallback retains a local ownership secret. It is not silently
merged into a different WOU-ID account: a future migration must prove both
owners before moving progress. Signing out switches the client to an anonymous
session; it does not delete the account.

The browser shell and Android wrapper consume the same account contract.
Android's Play Games adapter is a provider-specific edge; it does not own
account creation.

## Web OAuth callbacks

SOW uses `https://shadowsofwar.io/auth/callback` directly for Google, Discord,
Meta, and X. The same provider clients keep
`https://worldofunreal.com/auth/callback` for the central hub. Both exact URLs
are served by WOU-ID's public OAuth callback configuration and must be
registered in the provider consoles. Play Games is Android-native and does not
use either web callback.

Each login attempt carries a short-lived state value. WOU-ID pins that state to
the provider and callback, consumes it once, and requires S256 PKCE for X.

## Commerce contract

- RevenueCat receives `account_id` as `app_user_id`.
- Stripe checkout and webhook metadata use the same `account_id`.
- Android uses Google Play Billing through RevenueCat.
- Web uses the approved RevenueCat/Stripe web checkout.
- The server verifies the account before granting content and deduplicates
  provider events by transaction/event identity.

No purchase path accepts a display name, provider subject, or alternate public
account identifier.

## Test-data reset

The operator-only reset endpoint has two safeguards:

- It defaults to a dry run and reports human accounts, preserved bots, rows,
  keys, and purchase state.
- A live reset requires the exact confirmation
  `RESET_HUMAN_TEST_DATA` and refuses to run if any human purchase record is
  present.

The reset removes human test records from WOU-ID and SOW while preserving bot
accounts, game catalogs, and provider configuration. It uses Redb and Valkey
APIs; it never edits database bytes directly.

## Adding a provider

Add one provider adapter in WOU-ID, verify its token there, and return the
existing `account_id` contract. Do not add a new account field to SOW and do
not create a second game database.

## Release gates

- `./wou deploy` updates WOU-ID through its official pipeline.
- `./sow p` updates SOW Web/backend/infrastructure through its official
  pipeline.
- `scripts/android-local-test.sh` validates the Android wrapper locally.
- `./sow a` remains the owner-run Android/Google Play upload.
