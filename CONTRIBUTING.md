# Contributing to Shadows of War

Thank you for your interest in contributing!

For local launch instructions, see the [README](README.md#run-locally).

## Relay builds and upgrades

The relay uses the F-Stack `v2.0` source selected by `SOW_FSTACK_REPO` and its
bundled DPDK 24.11.8. The [production builder](sow-dist/src/prod.rs) applies the
[tracked integration patch](fstack-bridge/patches/fstack-2.0.patch), builds DPDK
in a private prefix inside the existing Debian 12 builder, and checks the
native interface before linking. It does not use the builder's system DPDK.
The process/queue layout and packet options live in
[the relay configuration](fstack-bridge/echo-vf.ini).

Compatible application updates use F-Stack's shared handover protocol. Existing
matches, reconnect credentials, TLS sessions and pending turn deliveries remain
with their original workers; new matches go to the replacement generation.
Library, configuration, or supervisor changes require an empty match registry
and a controlled restart. Old workers have no match-duration shutdown deadline.
The existing production acceptance test checks continuous old-player traffic,
new-match ownership, reconnects, and confirmed rewards across a live handover.
Deployment continues to use `./sow p` only when requested by the owner.

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

- **Player-facing UI:** keep the OpenFront credit and exact source/license links in the main menu.
- **Marketing HTML:** same footer pattern as `sow-web/site/`; do not add the copyright holder’s personal name.
- **Legal notices:** update the root `LICENSE`, `docs/legal/NOTICE`, `docs/legal/LICENSE-ASSETS`, or `assets/maps/NOTICE` at its existing source.
- **Upstream OpenFront:** legal entity is OpenFront Inc. and Contributors (see their LICENSING.md); player UI uses “OpenFront and Contributors”.

## Code of conduct

Be respectful and constructive. We want a welcoming community for everyone.
