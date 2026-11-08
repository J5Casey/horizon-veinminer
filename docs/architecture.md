# Architecture

```
mixins/ultimine/                 the bundle (what the .hnmod carries)
  horizon-mixin.toml             [mod], [compatibility] pin, [api] path = "api"
  api/                           the trait mod package the launcher links in
    src/engine/                  the engine: no Horizon types at all
    src/lib.rs                   `hooks()`, the Hooks impl: routes hooks to the halves
    src/client_side.rs           key, wheel, tick (messages + preview), visuals, hud
    src/server_side.rs           messages, rules, destroy_block, right_click_block
    src/world.rs                 the engine's traits over `dyn Host`
    src/player_break.rs          the one seam for "break as the player"
Cargo.toml                       builds src/engine alone (crate `ultimine-engine`)
tests/                           engine tests (shapes, server, client, right click)
```

## Engine

`engine/` is plain Rust over a few traits, compiled twice: as the root crate for
`cargo test`, and as `mod-ultimine`'s `engine` module in the game.

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
- `protocol`: versioned JSON on `ultimine:state` (client to server) and
  `ultimine:rules` (server to client), size-checked.
- `config`, `page`: the rules and preferences, and the settings page.

## Glue

`lib.rs` holds one `Ultimine` with a `Mutex` per half: one instance serves the
window's game and the server's. The client half reads the key with
`Host::mod_key`, runs its tick on the window (`Host::window`), sends state with
`net_send_to_server`, previews from `view_hit` and client block reads, outlines
with `visuals` rims and draws the overlay in `hud`.

The server half is authoritative. `left_click_block` records the face a dig
started on; `destroy_block` (acting player swapped in) asks the engine, selects
again from the server's world, and when more than one block is selected breaks
them all, the origin first, through `player_break::BREAK`, then cancels the
original break, as Ultimine cancels its `BreakEvent`. Those breaks reach
`destroy_block` again: a thread-local guard makes them pass through, and the
server hooks check it before taking their lock.

## The player-break seam

Breaking "as the player" (their checks, the hook, loot for the held tool, tool
wear, statistics, exhaustion, experience, optionally into the inventory) is
`Host::player_break_block(uuid, pos, into_inventory)`, which is being added to
Horizon (horizon-rust/horizon#12259). The api crate's default feature
`player-break` uses it; without the feature it builds an interim break
(`Host::destroy_block`, a machine break with the held tool's loot: no tool wear,
statistics or drops-to-inventory) for a Horizon that lacks the helper.
`player_break_block` fires `destroy_block` for each block it breaks, so Ultimine
holds `engine::reentry::Guard` while it breaks and its hooks pass those breaks
through.
