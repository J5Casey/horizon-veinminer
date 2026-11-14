# Ultimine for Horizon

An [FTB Ultimine](https://github.com/FTBTeam/FTB-Ultimine)-style vein miner for
[Horizon](https://github.com/arismoko/horizon) (Minecraft 26.3 in Rust), as a
native mod on the mod API alone: no injections into private game functions.

Hold the Ultimine key (default <kbd>`</kbd>, rebindable under
Controls) and break a block: everything the current shape selects breaks with it, as you, with your tool. Scroll while holding the key to change shape. The blocks
that will break are outlined, and an overlay names the shape and the count.

- **Shapes** (Ultimine's six, in its order): Shapeless (follows the vein, the
  default), Small Tunnel, Small Square (3x3), Large Tunnel (3x3), Mining Tunnel
  (diagonal down) and Escape Tunnel (diagonal up).
- **Matching**: shapeless mining takes the same block, or a merge group (ores
  with their deepslate variant, overworld stone, a tree species' logs and wood);
  shaped mining takes any block by default, never air, fluids, unbreakable or
  much harder blocks (obsidian behind stone stays).
- **Tree felling** is shapeless mining on a log: the whole connected trunk and
  branches of that species come down. No separate tree mod is needed.
- **Right click** with the key held: axes strip, shovels make paths, hoes till,
  and ripe crops are harvested and replanted (one seed each).
- **Costs and limits** follow Ultimine's defaults: 64 blocks, 20 hunger per block
  (0.1 exhaustion), stop when the tool breaks or food runs out. A vein's drops
  and ore experience land together at the block you mined, merged into as few
  stacks as fit, as in FTB Ultimine; or they go straight to the inventory, with
  what doesn't fit landing there.
- **Multiplayer**: the client sends its key state and shape; the server decides
  and breaks. Works in single player, on LAN and on dedicated servers.

## Build and install

A native mod for [arismoko/horizon](https://github.com/arismoko/horizon): the
flat layout (`horizon-mod.toml`, `Cargo.toml`, `src/`, `assets/`) that the
Horizon Fork Launcher compiles against the prebuilt SDK of the commit
`horizon-mod.toml` pins. Pack it into a `.hmod` (Braid's `tools/braid_mods.py
--fork` does) and build with `horizon-fork-launcher native-build
--trust-native-code`. Settings are under Options > Mods > Ultimine, the key
under Controls.

## Development

```sh
(cd engine && cargo test && cargo clippy --all-targets -- -D warnings)  # the engine, without the game
# The mod against the SDK: link a checkout of the pinned commit first.
mkdir -p .horizon/sdk && ln -s <horizon checkout> .horizon/sdk/api
cargo clippy --all-targets -- -D warnings
```

See [architecture](docs/architecture.md) and
[feature coverage and compatibility](docs/compatibility.md).

## Credits

Forked from [levzzz5154/horizon-veinminer](https://github.com/levzzz5154/horizon-veinminer),
an unofficial Rust port of [MiraculixxT's Veinminer](https://github.com/MiraculixxT/Veinminer).
Behaviour modelled on FTB Ultimine by Feed The Beast; this repository contains
independently written code and no code, artwork or assets from either project or
from Minecraft. License: [MIT](LICENSE).
