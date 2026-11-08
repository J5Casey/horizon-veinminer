//! Breaking a block as the player: the single engine call the mod depends on
//! that Horizon's stable API doesn't have yet. Everything else goes through
//! catalog hooks and helpers; this is the seam where the engine's own
//! `Host::player_break_block` (horizon-rust/horizon#12195) plugs in.
use crate::engine::geom::Pos;
use horizon_client::mod_api::Host;

/// Breaks `pos` as player `uuid` breaks it: their checks, the
/// `destroy_block` hook, loot for the held tool, tool wear, statistics,
/// exhaustion, experience. With `into_inventory` the drops go to the player.
/// True when the block was removed.
pub(crate) trait PlayerBreak: Sync {
    fn break_block(&self, host: &mut dyn Host, uuid: u128, pos: Pos, into_inventory: bool) -> bool;
}

/// Horizon's own player break (`native_player_break_block`).
#[cfg(feature = "player-break")]
pub(crate) struct EngineBreak;

#[cfg(feature = "player-break")]
impl PlayerBreak for EngineBreak {
    fn break_block(&self, host: &mut dyn Host, uuid: u128, pos: Pos, into_inventory: bool) -> bool {
        host.player_break_block(uuid, pos, into_inventory)
    }
}

/// Interim, until the engine helper is merged: `Level.destroyBlock` with the
/// loot rolled for the held tool (`native_machine_destroy_block`). Drops
/// and ore experience are right; tool wear, statistics and drops to the
/// inventory are not, and nested `destroy_block` hooks don't fire.
#[cfg(not(feature = "player-break"))]
pub(crate) struct InterimBreak;

#[cfg(not(feature = "player-break"))]
impl PlayerBreak for InterimBreak {
    fn break_block(
        &self,
        host: &mut dyn Host,
        _uuid: u128,
        pos: Pos,
        _into_inventory: bool,
    ) -> bool {
        use horizon_client::mod_api::EquipmentSlot;
        let tool = host.equipment(EquipmentSlot::MainHand);
        host.destroy_block(pos, tool.as_ref(), false).is_some()
    }
}

#[cfg(feature = "player-break")]
pub(crate) const BREAK: &dyn PlayerBreak = &EngineBreak;
#[cfg(not(feature = "player-break"))]
pub(crate) const BREAK: &dyn PlayerBreak = &InterimBreak;
