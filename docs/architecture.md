# Architecture

```
horizon-mod.toml                 the manifest: id, [native] pins (arismoko/horizon)
Cargo.toml                       the mod crate (depends on horizon-mod-api only)
assets/ultimine/lang/            the settings page's and the key's labels
src/engine/                      the engine: no Horizon types at all
src/lib.rs                       the Hooks impl: registration, routes hooks to the halves
src/client_side.rs               key, wheel, tick (state + preview), visuals, overlay
src/server_side.rs               state, rules, break_block, right_click_block
src/gather.rs                    the vein's drops and experience, kept by block_drops
src/world.rs                     the engine's traits over the server host
engine/Cargo.toml                builds src/engine alone (crate `ultimine-engine`) for tests
tests/                           engine tests (shapes, server, client, right click)
```

## Engine

`src/engine/` is plain Rust over a few traits, compiled twice: by `engine/Cargo.toml`
for `cargo test`, and as `mod-ultimine`'s `engine` module in the game.

- `geom`: positions, faces (`Face::from_yaw`, `Face::from_hit`), checked offsets.
- `shape`: the `Shape` trait and the six shapes. A shape gets the origin, the face
  hit, the player's horizontal facing, the block limit and a `check(pos)`; it
  returns positions in breaking order. `Shapeless` is a bounded breadth-first
  flood over the 26 neighbours (Ultimine's plant neighbourhood for plants),
  nearest first; `Line` is the small tunnel; `Layers` (depth 1 or unlimited) the
  small square and large tunnel; `DiagonalTunnel { rise }` the mining and escape
  tunnels.
- `matcher`: Ultimine's choice of matcher (merge group, plant kind, same block,
  or any block for shaped mining) and the exclusions. Tags go through `Tags`.
- `select`: one `select(world, rules, tags, request)` used by both halves.
- `server`: per-player state from the client, `Server::request` (active?, game
  mode, cooldown, food, tool, which face), and `execute`, the break loop over
  `Breaker`: snapshot, skip changed blocks, break as the player, exhaustion,
  stop when the tool breaks or food runs out.
- `client`: activation (key / sneak / both), shape cycling, the messages to send
  and the preview.
- `rightclick`: strip/path/till/harvest decisions over `Farmer`.
- `protocol`: versioned `key=value` text on the registered payloads
  `ultimine:state` (client to server) and `ultimine:rules` (server to client),
  size-checked. No serde: a native mod gets only the SDK.
- `config`, `page`: the rules and preferences, and the typed settings that hold
  them (Options > Mods > Ultimine).

## Glue

`lib.rs` holds one `Ultimine` with a `Mutex` per half: one instance serves the
window's game and the server's. `register` declares the settings, the key
mapping and the two payloads. The client half polls the key with `is_down` in
`client_tick_post`, sends state with `send_payload_to_server`, previews from
`view_hit` and client block reads, outlines with `visuals` rims and draws the
overlay in its own GUI layer (`ultimine:overlay`).

The server half is authoritative. `left_click_block` records the face a dig
started on; `break_block` (acting player from `players().acting_player()`) asks
the engine, selects again from the server's world, and when more than one block
is selected breaks them all, the origin first, with `PlayerRef::destroy_block`
(arismoko/horizon#4143), then cancels the original break, as Ultimine cancels
its `BreakEvent`. Those breaks reach `break_block` again: a thread-local guard
makes them pass through, and the server hooks check it before taking their lock.

## Drops at the origin

`PlayerRef::destroy_block` is the player's own break: their checks, loot for the
held tool and its enchantments, tool wear, the mined statistic, exhaustion. Each
of a vein's breaks fires `block_drops` with the player as the breaker; `gather`
(its own lock, since the server half's is held) takes the item list and the
experience of every block the use broke. When the use ends
(`Breaker::drop_gathered`), `world::set_down` merges the stacks into as few as
fit (`is_stackable_with`, up to `max_stack_size`) and pops them at the origin
with `LevelRef::pop_resource`, and awards the experience at the origin's centre
with `award_experience`. With `drops_to_inventory` the merged stacks go into the
main inventory first (`ItemPlace::PlayerSlot` inserts in a transaction, onto
matching stacks, then empty slots, as `Inventory.add`), and only what doesn't
fit lands at the origin. Right-click harvesting breaks with `destroy_block` and
`into_inventory` directly, so a crop's drops land where it grew (or go to the
inventory).
