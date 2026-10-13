# Web checkout runbook

This procedure covers the existing Shadows of War Stripe account and
RevenueCat project.

## Credentials

Keep the four checkout values in the ignored sow-dist/.env file with mode 0600:

    SOW_STRIPE_SECRET_KEY=sk_live_...
    SOW_STRIPE_PUBLISHABLE_KEY=pk_live_...
    SOW_STRIPE_WEBHOOK_SECRET=whsec_...
    SOW_REVENUECAT_STRIPE_API_KEY=strp_...

The RevenueCat CLI credential at
/home/bizkit/.config/shadows-of-war/revenuecat_api_key is separate; it does not
replace SOW_REVENUECAT_STRIPE_API_KEY.

## Release and recovery

The production pipeline requires all four values. Run ./sow p to publish Web
and backend changes; it synchronizes the configured runtime copies. Before
calling checkout production-ready, confirm the pipeline health checks, public
catalog, and Stripe webhook.

If a secret is lost, retrieve or rotate it through the provider. Replace only
the value that changed, preserve the other working values, and run ./sow p.
Rotation is a separate operation. If it is deferred, keep checkout enabled and
continue unrelated releases.
