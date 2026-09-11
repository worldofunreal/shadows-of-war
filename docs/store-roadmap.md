# Shadows of War — store decision log

## Decided for the first Android slice

- Main menu entry: `Store`.
- Eight leaders are free in a deterministic weekly rotation.
- A new player receives one random leader from that rotation.
- Leaders outside the rotation are locked until the player unlocks them with 500 crowns or 1,500 gems.
- Leader unlocks offer either currency; the server owns both balances and validates the selected spend.
- The server resolves the selected leader before a match, so a client cannot use a locked leader by editing local state.
- The purchase surface is universal: in-game store → platform checkout/RevenueCat → server grant. It is not tied to CrazyGames or Poki.
- Android gem bundles use Google Play Billing through the native RevenueCat bridge. Product IDs are `sow_gems_500`, `sow_gems_1200`, and `sow_gems_2600`; RevenueCat receives the public profile ID, never the private account ID.
- Android release is intentionally separate: `./sow a` builds, device-tests, validates, and uploads the AAB to Play Alpha; `./sow p` never uploads Android.
- RevenueCat purchase events are granted by the server and deduplicated by the
  provider transaction ID when available, with the event ID as fallback.
- Authenticated purchase history is available through `POST /store/purchases`
  and `POST /store/purchases/{id}`; the anonymous ownership secret is required
  and is never returned in the response.
- Each delivered purchase records provider, environment, product, transaction,
  and status. RevenueCat refunds/voids are idempotent; direct leader/skin
  ownership is removed only when that purchase was the source of ownership.
- Stripe Checkout sessions are recorded as `pending` before payment and move
  through `paid`/`submitted`, `failed`, or `expired` from signed webhook events;
  a restart does not erase an unresolved purchase.

## Implemented in the first Android slice

- Three original SOW skins are catalogued, previewed, unlocked with gems, and equipped through authenticated server endpoints.
- Skin prices are 500 / 1,000 / 1,500 gems for the launch catalog; the 2,600-gem Kingdom Vault cannot buy all three skins.
- The end-of-match screen shows the next unowned skin as a featured store offer and can open the store after returning to the menu.

## Current release state

- Android gem bundles are registered in Google Play and attached to the same
  RevenueCat offering as the three Stripe web products. The production AAB
  currently contains the native RevenueCat purchase bridge.
- Web uses an in-game Stripe Embedded Checkout session. The client sends the
  authenticated public profile ID and product ID to `/store/checkout`; the
  private account secret never leaves the game. Stripe completion is imported
  into the same RevenueCat project, whose webhook performs the authoritative
  grant.
- The Apple App Store app uses the same RevenueCat project with bundle ID
  `games.shadowsofwar.app`; the three consumables are configured and the
  native bridge is in the Xcode target. A Mac/TestFlight build is still
  required for final runtime purchase proof.

## Storefront and checkout contract

- The Shop front page contains featured gem offers and selected promotions;
  the complete leader catalogue remains in Heroes.
- Every purchase action is labelled `BUY`. Web checkout stays mounted in the
  Shop screen; Android opens the native Google Play confirmation surface.
- Direct leader and skin product IDs are
  `sow_offer_leader_<leader_id>` and `sow_offer_skin_<skin_id>`. Their Stripe
  and Play price mappings are runtime configuration, never guessed in source.
- Direct leader and skin buttons remain hidden until the corresponding Stripe
  price and Google Play/App Store products exist and are linked in RevenueCat.
  The current live catalog contains only the three gem products, so exposing
  those placeholder IDs would create checkout buttons that cannot complete.
- Production checkout requires `SOW_STRIPE_SECRET_KEY`,
  `SOW_STRIPE_PUBLISHABLE_KEY`, `SOW_STRIPE_WEBHOOK_SECRET`, and
  `SOW_REVENUECAT_STRIPE_API_KEY`.
- The Android TWA bridge uses the existing Custom Tabs session, the
  `common.use_as_origin` Digital Asset Links relation, and a correlated
  `purchase_result` message. It must not navigate the TWA back to `/play/`.

## Remaining release work

- Validate one Google Play purchase on-device and one embedded Stripe purchase
  end to end, including the idempotent server grant.
- Build the iOS app with the RevenueCat public SDK key supplied through the
  signed app configuration, then validate one sandbox purchase plus the
  idempotent server grant.
- Balance leader prices after real retention and economy data exists.

The current leaders have gameplay perks. Selling access to them is intentional under the rotation model, but future hackathon/store copy must not claim that every purchase is purely cosmetic.

OpenFrontIO is a behavior reference only. Do not copy its proprietary/premium assets or code; use original Shadows of War assets and the small contract above.
