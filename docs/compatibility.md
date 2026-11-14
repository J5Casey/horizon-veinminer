# Feature coverage and compatibility

## Against FTB Ultimine

| Ultimine | Here |
|---|---|
| Hold key (default `` ` ``), rebindable | Yes (Controls) |
| Shapeless, small tunnel, small square, large tunnel, mining tunnel, escape tunnel | Yes, same geometry and order |
| Scroll to cycle shapes, overlay with context lines | Yes (two lines each way) |
| Outline preview (`render_outline` 256) | Yes, Horizon's block rims, 256 per mod; the targeted block's too, in place of its vanilla outline |
| `merge_tags` / `merge_tags_shaped = ["*"]` | Merge groups of ids and `#tags`; shaped any-block default |
| Crop-like matching with the plant neighbourhood | Yes, by plant kind |
| `max_blocks` 64, `exhaustion_per_block` 20, cooldown, `require_tool` | Yes |
| Hardness check (no obsidian behind stone) | Approximated: a list of hard blocks is never taken by any-block matching |
| Stop when the tool breaks or the player starves | Yes |
| Drops accumulated at the origin | Yes, merged, with the ore experience; or to the inventory first (`drops_to_inventory`) |
| Right click: strip, path, till, harvest crops | Yes; copper scraping and FTB EZ Crystals are not |
| `excluded_blocks` / `block_whitelist` tags | `excluded_blocks` / `only_blocks` lists |
| Experience cost per block, FTB Ranks, attributes | No |
| Keyboard shape cycling keys | No (wheel only) |

The client preview has no tag data: groups written as `#tags` only match on the
server. The default groups are written out as block ids so preview and server
agree. Right-click actions don't wear the tool.

## Horizon API used (arismoko/horizon)

Pinned in `horizon-mod.toml` to `24715f9161`, J5Casey/horizon `braid/ari-base`:
arismoko/horizon main `d221ff1d6f` plus #4137 and #4143 (`PlayerRef::destroy_block`,
open). Once #4143 merges, repin to a main commit with a `prebuilt-` SDK.

Hooks: `register` (settings, key mapping, payloads), `register_gui_layers`,
`gui_layer_draw`, `setting_changed`, `client_tick_post`, `mouse_scrolling`,
`visuals`, `block_highlight`, `server_tick`, `player_logged_out`,
`left_click_block`, `break_block`, `block_drops`, `right_click_block`.

Host: `is_down`, `playing`, `local_player`, `view_hit`, `setting`/`set_setting`,
`send_payload_to_server`, `PlayerRef::send_payload`, `players().acting_player()`,
`PlayerRef::{level, game_mode, food, inventory, add_exhaustion, destroy_block}`,
`EntityRef::{rotation, is_shift_key_down}`, `LivingRef::equipment`,
`LevelRef::{block, block_property, set_block, pop_resource, award_experience}`,
`block_key`, `block_in_tag`, `block_state_default`, `set_block_property`,
`max_stack_size`, `ItemPlace::{insert, extract}` in `transaction`, and the
draw context's `fill`, `text`, `text_width`.

What changed in the port (0.5.0):

- The settings page's custom buttons are typed settings now (toggles and
  choices, same options). The rules' hand-edited lists (`excluded_blocks`,
  `only_blocks`, custom merge groups) had only a JSON file behind them; that
  file is gone with serde (the launcher gives a native mod the SDK only), so
  they keep their defaults until a config file comes back.
- The overlay sits top left in its own GUI layer; there is no `Corners`
  placement helper on arismoko, so it can overlap another mod's top-left text.

`PlayerRef::destroy_block` fires `break_block` again for each block it breaks,
as any player's break does. Ultimine holds a guard (`engine/reentry.rs`) while
it breaks, so those nested hooks pass the break through and never start another
Ultimine use (`tests/reentry.rs`).

## Checked in game

See the commit that adds this port's in-game check, and the history for the
horizon-rust era checks (0.4.x on `a3bf653`).

## Not yet validated

LAN guests and dedicated servers run the same server code but were not tried.
