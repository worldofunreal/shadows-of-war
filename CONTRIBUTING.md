# Contributing to Shadows of War

Thank you for your interest in contributing!

## Getting started

1. Fork the repository and clone your fork.
2. Vendor the pinned graphics fork (gitignored, not vendored): `./scripts/vendor-blade.sh`.
3. Ensure `assets/gameplay/` is complete (fonts, emoji, HUD assets, and avatars are required by the gameplay client).
4. Build. The full workspace includes `fstack-bridge` and `sow-relay`, which link `libfstack.a` + DPDK and only build on the FreeBSD production host. On a normal dev machine, build the game crates:
   ```bash
   cargo build -p sow-core -p sow-data -p sow-client -p sow-server -p sow-render -p sow-net -p sow-audio -p sow-i18n -p sow-map
   ```
5. Run tests (the pipeline's own gate):
   ```bash
   cargo test -p sow-core
   cargo test -p sow-data --features server
   cargo test -p sow-server
   ```
6. Format and lint before submitting:
   ```bash
   cargo fmt --all
   cargo clippy -p sow-core -p sow-client -p sow-server -- -D warnings
   ```

## Launching the game (`./sow`)

The single entrypoint is the `sow` launcher at the repo root:

- `./sow` or `./sow native` — fast-build and open the native Tauri/WASM desktop client.
- `./sow l` (or `./sow local`) — build WASM + webroot and serve the browser client at `http://127.0.0.1:4173`.
- `./sow m` — serve the campaign editor locally on port 8777.
- `./sow p` — production build & deploy (maintainers only).
- `./sow a` — Android AAB + Play alpha upload (maintainers only).
- `./sow jest` — build the self-contained Jest bundle (maintainers only).

## Marketing site (`sow-web`)

Static HTML lives in `sow-web/site/` (landing, privacy, terms, cookies, support). The WASM game shell lives in `sow-web/shell/`. Do not embed game code in the marketing pages.

## Pull requests

- Keep changes focused — one logical change per PR when possible.
- Update `assets/SOURCES.toml` and `assets/maps/SOURCES.toml` if you add or replace assets.
- New shipped art (portraits, avatars): default to **CC BY-SA 4.0** per `docs/legal/LICENSE-ASSETS`; verify AI tool ToS before setting `license = "CC-BY-SA-4.0"`.
- Document OSM-derived maps with `source = "osm"` and attribution in SOURCES.toml.
- Do not commit secrets, `sow-dist/deploy/keystores/`, `.env` files, or `OpenFrontIO/proprietary/` assets.

## License

By contributing, you agree that your contributions will be licensed under the
[GNU Affero General Public License v3.0 only](LICENSE), with the OpenFront
additional terms included there.

Inbound contributions = outbound under AGPL-3.0-only. No separate CLA is required.

See also “License & derivatives” in the README for obligations that apply to anyone deploying a derivative, including network use under AGPL section 13.

## Attribution

- **Player-facing UI:** brand (`© Shadows of War`), OpenFront attribution on the main menu, full notices in Credits.
- **Marketing HTML:** same footer pattern as `sow-web/site/`; do not add the copyright holder’s personal name.
- **Legal files:** update only under `docs/legal/` (COPYRIGHT, NOTICE, LICENSE-ASSETS).
- **Upstream OpenFront:** legal entity is OpenFront Inc. and Contributors (see their LICENSING.md); player UI uses “OpenFront and Contributors”.

## Code of conduct

Be respectful and constructive. We want a welcoming community for everyone.

## Faster debug builds

For UI-only work, you can speed up clean debug compiles by lowering dependency optimization in a local `.cargo/config.toml` (not committed):

```toml
[profile.dev.package."*"]
opt-level = 1
```

The workspace default uses `opt-level = 3` for dependencies so debug runs stay smooth; lowering it trades runtime speed for compile time.

Native dev tools (FPS overlay, dev sidebar, map shader sliders) are gated in `sow-client` by `cfg(any(feature = "dev", debug_assertions))`. The `./sow` launcher does not currently pass `--features dev`; enable dev tooling by building the client manually with that feature flag.

The browser shell owns the menus and debug controls; the native desktop package
only provides the Tauri window around that same client.
