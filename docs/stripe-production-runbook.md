# Shadows of War Stripe production runbook

This is the single reference for the live Web checkout. It applies to the
existing Shadows of War Stripe account and the existing RevenueCat project.

## Credential ownership

The four checkout values live only in the ignored file
`sow-dist/.env`, with mode `0600`:

```env
SOW_STRIPE_SECRET_KEY=sk_live_...
SOW_STRIPE_PUBLISHABLE_KEY=pk_live_...
SOW_STRIPE_WEBHOOK_SECRET=whsec_...
SOW_REVENUECAT_STRIPE_API_KEY=strp_...
```

The file `/home/bizkit/.config/shadows-of-war/revenuecat_api_key` is the
RevenueCat CLI credential. It is not a replacement for
`SOW_REVENUECAT_STRIPE_API_KEY`.

Never commit, print, paste, or place these values in source, Android, Web, or
documentation. The Stripe MCP can operate the authorized account, but it does
not recover secret API keys or webhook signing secrets.

## Poka-yoke in `./sow p`

The production pipeline requires all four values before any build or deploy.
It fails closed when a value is missing, empty, or not in live mode:

- `sk_live_` — Stripe server API key.
- `pk_live_` — browser publishable key.
- `whsec_` — signing secret for the existing production webhook.
- `strp_` — RevenueCat Stripe app key used for external purchase import.

This prevents a partial or accidentally cleared local configuration from
silently publishing a checkout-disabled release. The pipeline also compares
SHA-256 fingerprints with the three runtime copies and backs up each remote
`sow.env` before synchronization. It never logs the credential values.

## Normal workflow

1. Keep the four values in `sow-dist/.env`; do not send them through chat.
2. Run the read-only Stripe account check when validating a replacement
   `sk_live_` key.
3. Run `./sow p` for Web, backend, and infrastructure. It does not upload
   Android.
4. Confirm pipeline health checks, public catalog availability, and the
   Stripe webhook endpoint before calling checkout production-ready.

## Recovery and rotation

If a secret is lost, retrieve or rotate it in the relevant provider dashboard;
the MCP and source repository cannot reconstruct it. After rotating a Stripe
secret or webhook signing secret, update all four values together and run
`./sow p` so the runtime copies stay synchronized.

Rotation is an explicit security operation, not a normal deployment step. If
rotation is deferred, keep the current working credentials, do not disable
checkout, and do not stop the deployment. If rotation is requested, update the
credential bundle once and run `./sow p`.

If a credential is pasted into chat or another untrusted location, record that
as a security recommendation for later rotation; it is not a reason to delete
the working value or block unrelated work.

## Agent continuity contract

When context is compacted or a new agent takes over:

1. Inspect `sow-dist/.env` and its mode before asking for any credential.
2. Check the configured MCPs and CLIs before claiming access is missing.
3. Preserve every existing value; never clear configuration to make a partial
   setup look clean.
4. Ask the user only for a variable that remains genuinely absent after those
   checks.
