# Contributing

Run the game with the commands in the [README](README.md#run-locally).

## Pull requests

- One logical change per PR.
- Record new or replaced assets in `assets/SOURCES.toml`, and maps in `assets/maps/SOURCES.toml`. OSM-derived maps use `source = "osm"` with attribution.
- New shipped art defaults to CC BY-SA 4.0 (`docs/legal/LICENSE-ASSETS`). Check the AI tool's terms before marking generated art that way.
- Never commit secrets, `.env` files or `sow-dist/deploy/keystores/`.

## License

Contributions are licensed under [AGPL-3.0-only](LICENSE) with the OpenFront additional terms. No CLA.

## Attribution

- Keep the OpenFront credit and the source and license links in the main menu.
- Site pages in `sow-web/site/` use the same footer. Do not add the copyright holder's personal name.
- Update legal notices where they live: `LICENSE`, `docs/legal/NOTICE`, `docs/legal/LICENSE-ASSETS`, `assets/maps/NOTICE`.
- Credit upstream as "OpenFront and Contributors".

Be respectful.
