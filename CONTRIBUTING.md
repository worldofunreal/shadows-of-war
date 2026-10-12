# Contributing to Shadows of War

Thank you for your interest in contributing!

## Local workflow

- `./sow` or `./sow native` opens the native client.
- `./sow l` or `./sow local` builds and serves the browser client at
  `http://127.0.0.1:4173` for local visual review; it is not the release gate.
- Maintainers use `./sow p` for Web, backend, and infrastructure releases.
- Android signing and Play uploads are handled by the owner through `./sow a`.

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
