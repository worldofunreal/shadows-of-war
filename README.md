<p align="center">
  <img src="assets/shell/loader/sow-splash-desktop.webp" alt="Shadows of War" width="100%" />
</p>

<h1 align="center">Shadows of War</h1>

<p align="center">
  <strong>Command one of hundreds of legendary historic leaders in real-time territory wars.</strong><br />
  Build cities, factories and ports, forge alliances, betray rivals, and redraw the world map in this multiplayer game.
</p>

<p align="center">
  <a href="https://shadowsofwar.io/play/"><strong>▶ Play now</strong></a>
  · <a href="https://shadowsofwar.io/how-to-play/">How to play</a>
  · <a href="https://discord.gg/d6ZDeChSE">Discord</a>
</p>

<p align="center">
  <a href="https://github.com/worldofunreal/shadows-of-war/blob/master/LICENSE"><img src="https://img.shields.io/badge/license-AGPL--3.0-blue" alt="AGPL-3.0 license" /></a>
  <img src="https://img.shields.io/badge/client-Rust%20%2B%20WASM-orange" alt="Rust and WebAssembly client" />
  <img src="https://img.shields.io/badge/interface-JavaScript-f7df1e" alt="JavaScript interface" />
  <img src="https://img.shields.io/badge/rendering-Blade%20GPU-111827" alt="Blade GPU rendering" />
</p>

## The game

![World map](assets/site/media/session-world.webp)

Shadows of War is a multiplayer strategy game about taking territory and turning it into power.

- Expand across a living world map.
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
| `sow-client` | Rust/WASM client and Blade rendering integration |
| `sow-web/shell` | JavaScript interface, HUD, tutorial and browser input |
| `sow-native` | Tauri desktop shell for the JavaScript/WASM client |
| `sow-data` | Leaders, maps, profiles and game data |
| `sow-server` | Matchmaking and multiplayer coordination |
| `sow-relay` | Multiplayer relay service |
| `sow-dist` | Build and packaging pipeline behind `./sow` |

## Links

- [Play Shadows of War](https://shadowsofwar.io/play/)
- [How to play](https://shadowsofwar.io/how-to-play/)
- [Contributing](CONTRIBUTING.md)
- [License](LICENSE)

## License

Shadows of War is licensed under [AGPL-3.0-or-later](LICENSE).

Portions derive from [OpenFrontIO](https://github.com/openfrontio/OpenFrontIO). See [COPYRIGHT](docs/legal/COPYRIGHT) and [NOTICE](docs/legal/NOTICE).
