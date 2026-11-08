# Feature coverage and compatibility

## Against FTB Ultimine

| Ultimine | Here |
|---|---|
| Hold key (default `` ` ``), rebindable | Yes (Options > Mods > Ultimine) |
| Shapeless, small tunnel, small square, large tunnel, mining tunnel, escape tunnel | Yes, same geometry and order |
| Scroll to cycle shapes, overlay with context lines | Yes (two lines each way) |
| Outline preview (`render_outline` 256) | Yes, Horizon's block rims, 256 per mod |
| `merge_tags` / `merge_tags_shaped = ["*"]` | Merge groups of ids and `#tags`; shaped any-block default |
| Crop-like matching with the plant neighbourhood | Yes, by plant kind |
| `max_blocks` 64, `exhaustion_per_block` 20, cooldown, `require_tool` | Yes |
| Hardness check (no obsidian behind stone) | Approximated: a list of hard blocks is never taken by any-block matching |
| Stop when the tool breaks or the player starves | Yes |
| Drops accumulated at the origin | Drops pop at each block, or go to the inventory (`drops_to_inventory`) |
| Right click: strip, path, till, harvest crops | Yes; copper scraping and FTB EZ Crystals are not |
| `excluded_blocks` / `block_whitelist` tags | `excluded_blocks` / `only_blocks` lists |
| Experience cost per block, FTB Ranks, attributes | No |
| Keyboard shape cycling keys | No (wheel only) |

The client preview has no tag data: groups written as `#tags` only match on the
server. The default groups are written out as block ids so preview and server
agree. Right-click actions don't wear the tool.

## Horizon API used

Hooks: `mods`, `settings`, `settings_click`, `key`, `wheel`, `tick`, `visuals`,
`hud`, `net_from_client`, `net_from_server`, `player_logged_out`,
`left_click_block` (API 30), `destroy_block` (API 14), `right_click_block`
(API 21).

Helpers, stamped: `mod_key`, `setting`, `set_setting`, `window`, `playing`,
`player`, `view_hit` (116), `block` (20), `block_property` (20), `held`,
`acting_player` (21), `net_send_to_server` / `net_send_to_player` (40),
`set_block` (24), `set_block_property` (18), `take_items`, `fill`, `text`,
`text_width`.

Helpers still in pending fragments (no API number yet): `sneaking` and
`block_name_in_tag` (`host_bridge`), `player_food` and `set_player_status`
(`player_status`), `default_state` (`default_state`), and for the interim break
`equipment` (`host_bridge`) and `destroy_block` (`machine_breaking`); with
`player-break` (the default), `player_break_block` (`player_break`,
horizon-rust/horizon#12259, unmerged).

So the bundle pins `horizon_revision` to `aa13a46`, J5Casey/horizon
`braid/dev`: Horizon with #12259 (and the Braid pack's other unmerged engine
work, so every Braid mod builds on one commit). Once those fragments are
stamped and #12259 is merged: remove the pin from `horizon-mixin.toml` and the
api crate's `rev`, and add `"hooks": <the API that stamps the last of them>` to
`horizon-native.json` in `tools/package.py`.

`player_break_block` fires `destroy_block` again for each block it breaks, as
any player's break does. Ultimine holds a guard (`engine/reentry.rs`) while it
breaks, so those nested hooks pass the break through and never start another
Ultimine use (`tests/reentry.rs`).

## Not yet validated

The engine is unit-tested and the trait mod type-checks against the pinned
commit. In game (offscreen, single player, survival, `aa13a46` dev build): see
the 0.4.1 notes in the commit that re-pinned it. LAN guests and dedicated
servers run the same server code but were not tried.
