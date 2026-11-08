# Ultimine for Horizon

An [FTB Ultimine](https://github.com/FTBTeam/FTB-Ultimine)-style vein miner for
[Horizon](https://github.com/horizon-rust/horizon) (Minecraft 26.3 in Rust), as a
native **trait mod**: it uses only Horizon's catalog hooks and `Host` helpers, no
injections into private game functions.

Hold the Ultimine key (default <kbd>`</kbd>, rebindable under Options > Mods >
Ultimine) and break a block: everything the current shape selects breaks with it,
as you, with your tool. Scroll while holding the key to change shape. The blocks
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
  (0.1 exhaustion), stop when the tool breaks or food runs out. Drops can go
  straight to the inventory.
- **Multiplayer**: the client sends its key state and shape; the server decides
  and breaks. Works in single player, on LAN and on dedicated servers.

## Build and install

The mod is the bundle in [`mixins/ultimine/`](mixins/ultimine). Package it:

```sh
python3 tools/package.py   # writes target/ultimine-0.4.1.hnmod
```

Install the `.hnmod` with a launcher that supports native mods (Settings > Game >
Native mods), then build. Turn it off or on any time in Options > Mods.

## Development

```sh
cargo test                                  # the engine, without the game
cargo clippy --all-targets -- -D warnings
python3 tools/test_package.py
# Type-check the trait mod against a Horizon tree, staged as the launcher does:
git -C ../horizon archive <rev> | tar -x -C target/horizon-src
python3 tools/check_api.py target/horizon-src [--clippy] [--features player-break]
```

See [architecture](docs/architecture.md) and
[feature coverage and compatibility](docs/compatibility.md).

## Credits

Forked from [levzzz5154/horizon-veinminer](https://github.com/levzzz5154/horizon-veinminer),
an unofficial Rust port of [MiraculixxT's Veinminer](https://github.com/MiraculixxT/Veinminer).
Behaviour modelled on FTB Ultimine by Feed The Beast; this repository contains
independently written code and no code, artwork or assets from either project or
from Minecraft. License: [MIT](LICENSE).
