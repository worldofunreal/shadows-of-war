# Android runtime contract

This file is the anti-confusion gate for Android audits. It is the only current
behavior contract; old logs, screenshots, release notes, and agent summaries
are historical evidence until they are bound to the artifact under test.

## Required evidence identity

Every Android runtime result records all of:

- source commit SHA and whether the worktree was dirty;
- package, versionName, versionCode, Play track, artifact path, and artifact SHA-256;
- device serial/model/Android version;
- ISO timestamp;
- logcat and ordered runtime trace paths.

If the installed package cannot be bound to the source SHA and versionCode, the
result is UNKNOWN, not a conclusion about current behavior.

## Evidence labels

Use one label per statement:

- SOURCE: current repository code or configuration;
- DEVICE: observation from the identified installed artifact;
- PLAY_API: Google Play Publisher/Monetization API state;
- TRANSACTION: real tester purchase, RevenueCat webhook, server grant, or restore;
- HISTORICAL: older artifact, log, document, or release.

A source search never proves device behavior. A Play catalog response never
proves a purchase or grant. A successful build never proves a public release.

## Startup contract

The expected order is:

1. TwaLauncherActivity hands off directly to the TWA.
2. The TWA/game loader becomes visible.
3. The web loader emits sow:loader-ready.
4. The Custom Tabs bridge sends playgames_silent_auth.
5. Native code performs only silent Play Games authentication.
6. An existing session is exchanged through the rendezvous; otherwise the game
   continues anonymously.
7. Interactive Play Games UI appears only after an explicit user action.

The pre-WASM web step may restore cached identity state; it must not perform a
network identity handoff or initiate native Play Games authentication. The
native bridge request is the only silent-auth trigger.

Any interactive Play Games UI before the TWA or without a user action is a
runtime failure. The launcher must not call signIn, isAuthenticated, or
initialize Play Games before the loader bridge request.

## Commerce contract

- Android purchase and restore use the native RevenueCat/Google Play path.
- Android never opens the web RevenueCat/Stripe checkout link.
- The native bridge accepts only the fixed product IDs and validated public
  profile IDs.
- Purchase/restore results return with a request ID.
- Catalog state is PLAY_API; transaction, webhook, idempotent grant, restart,
  and restore are separate TRANSACTION checks.
- RevenueCat account switching uses its official identity lifecycle before a
  purchase is allowed.

## Audit gate

scripts/android-local-test.sh must fail when:

- the Android artifact identity is missing or mismatched;
- Java application sources remain;
- the launcher contains pre-loader authentication;
- the Android purchase branch contains an external checkout or lacks native
  purchase/restore and explicit sign-in paths;
- Play Games UI appears before TWA visibility or without an explicit tap;
- loader-ready, post-loader auth outcome, or final TWA visibility is absent.

The owner runs ./sow a for the signed AAB and Play upload. Codex does not run
that command or claim Play availability without PLAY_API evidence.
