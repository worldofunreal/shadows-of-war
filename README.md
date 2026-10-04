<p align="center">
  <img src="assets/shell/loader/sow-splash-desktop.webp" alt="Shadows of War" width="100%" />
</p>

<h1 align="center">Shadows of War</h1>

<p align="center">
  <strong>Command historic leaders in real-time territory wars. 12 playable today, roster growing.</strong><br />
  Build cities, factories and ports, forge alliances, betray rivals, and redraw the world map in this multiplayer game.
</p>

<p align="center">
  <a href="https://shadowsofwar.io/play/"><strong>▶ Play now</strong></a>
  · <a href="https://shadowsofwar.io/">Website</a>
  · <a href="https://discord.gg/d6ZDeChSE">Discord</a>
</p>

<p align="center">
  <a href="https://github.com/worldofunreal/shadows-of-war/blob/master/LICENSE"><img src="https://img.shields.io/badge/license-AGPL--3.0--only-blue" alt="AGPL-3.0-only license" /></a>
  <img src="https://img.shields.io/badge/client-Rust%20%2B%20WASM-orange" alt="Rust and WebAssembly client" />
  <img src="https://img.shields.io/badge/interface-JavaScript-f7df1e" alt="JavaScript interface" />
  <img src="https://img.shields.io/badge/rendering-Blade%20GPU-111827" alt="Blade GPU rendering" />
</p>

## The game

![World map](assets/site/media/session-world.webp)

Shadows of War is a multiplayer strategy game about taking territory and turning it into power.

- Expand across the world map.
- Build cities, factories, ports and an economy that supports your wars.
- Choose a historic leader and play around their strengths.
- Forge alliances, betray rivals and fight for control of the map.
- Join online matches or play local battles against bots.

| Territory | Battles | Expansion | Leaders |
| --- | --- | --- | --- |
| ![Territory](assets/site/media/session-world.webp) | ![Battle](assets/site/media/session-battle.webp) | ![Expansion](assets/site/media/session-expansion.webp) | ![Leaders](assets/site/media/session-leader.webp) |

## Built for the game

The project keeps the simulation, rendering and interface in separate layers:

- **Rust and WebAssembly** run the simulation, networking and game rules.
- **Blade** provides the dedicated GPU pipeline for the world, text, images and movers.
- **JavaScript** owns the menus, HUD, panels, tutorial and game interface.
- **Rust services** handle matchmaking, player data and multiplayer relay traffic.
- **Tauri** packages the same JavaScript/WASM client as the native desktop build.

There is no separate game engine layer. The game is built from its simulation, GPU pipeline and web interface.

## Run locally

```bash
./sow          # build and open the native JavaScript/WASM client
./sow native   # same native build
./sow l        # local browser preview at 127.0.0.1:4173
```

The native build and the browser preview use the same client. The desktop shell only packages the game; it does not create a second interface.

## Project map

| Path | Role |
| --- | --- |
| `sow-core` | Simulation rules and shared game state |
| `sow-client` | Rust/WASM game client and Blade integration |
| `sow-render` | Pure GPU rendering pipelines and shaders (Blade) |
| `sow-net` | WebSocket networking and turn relay protocol |
| `sow-audio` | Audio engine (WASM/cpal output, SFX preview) |
| `sow-i18n` | Locale catalogs and localization |
| `sow-map` | Map import, validation and thumbnail pipeline |
| `sow-data` | Leaders, maps, profiles, commerce and game data |
| `sow-server` | Matchmaking and multiplayer coordination |
| `sow-relay` | Multiplayer relay service (F-Stack/DPDK transport) |
| `fstack-bridge` | Zero-copy Rust bridge over F-Stack (FreeBSD only) |
| `sow-backfill` | Private-match backfill and bot-manager daemon |
| `sow-tools` | Map generator and tooling CLI |
| `sow-web/shell` | JavaScript interface, HUD, tutorial and browser input |
| `sow-native` | Tauri desktop shell for the JavaScript/WASM client |
| `sow-dist` | Build and packaging pipeline behind `./sow` |

## Links

- [Play Shadows of War](https://shadowsofwar.io/play/)
- [Website](https://shadowsofwar.io/)
- [Contributing](CONTRIBUTING.md)
- [License](LICENSE)

## License & derivatives

**Modified from OpenFrontIO. Source revision date: `2026-10-04`.** The in-game source link identifies the exact revision.

The integrated game code is licensed under [AGPL-3.0-only](LICENSE), with OpenFront's additional terms in section 7 preserved in that file. Map files derived from OpenFront are separately licensed under CC BY-SA 4.0; their sources, attribution, and changes are recorded in [assets/maps/SOURCES.toml](assets/maps/SOURCES.toml) and included in the served map notice. See [COPYRIGHT](docs/legal/COPYRIGHT) and [NOTICE](docs/legal/NOTICE).

**What this means if you build on this code:**

- You may use, study, modify and redistribute it — including commercially — under the same AGPL-3.0 license.
- If you run a modified version as a service over a network (a hosted game, a web client, or a relay), AGPL section 13 requires you to offer its **full corresponding source** to users who interact with it remotely.
- Derivative game code remains under AGPL-3.0-only, including the OpenFront additional terms. Map and other artwork keep the separate terms recorded in [docs/legal/LICENSE-ASSETS](docs/legal/LICENSE-ASSETS).
- The license does **not** grant the right to distribute your version under the *Shadows of War* name, logo or brand. Please rebrand derivatives entirely; we enforce our brand as a trademark matter separate from the code license.
- AGPL cannot require you to notify the upstream project, and we ask nothing of you as a license condition — but if you run something public built on this code, we would genuinely like to hear about it. Non-compliance (closed-source network use, stripped notices, brand reuse) is copyright infringement and we do monitor and enforce it.
