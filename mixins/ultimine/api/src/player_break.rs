//! Breaking a block as the player: the engine calls the mod depends on that
//! Horizon's stable API doesn't have yet. Everything else goes through
//! catalog hooks and helpers; this is the seam where the engine's own
//! `Host::player_break_block` (horizon-rust/horizon#12195) and its kept-drops
//! form (`Host::player_break_block_kept`, `Host::pop_break_drops`) plug in.
use crate::engine::geom::Pos;
use horizon_client::mod_api::{Host, ItemStack};

/// What one use's breaks set loose from the blocks themselves, gathered to
/// be set down at the origin together, as FTB Ultimine accumulates drops.
#[derive(Default)]
pub(crate) struct Gathered {
    pub stacks: Vec<ItemStack>,
    pub experience: u32,
}

/// Breaks `pos` as player `uuid` breaks it: their checks, the
/// `destroy_block` hook, loot for the held tool, tool wear, statistics,
/// exhaustion, experience. True when the block was removed.
pub(crate) trait PlayerBreak: Sync {
    /// With `into_inventory` the drops go to the player; otherwise they pop
    /// at `pos`.
    fn break_block(&self, host: &mut dyn Host, uuid: u128, pos: Pos, into_inventory: bool) -> bool;
    /// The block's own drops and experience go to `gathered` instead of
    /// landing at `pos` (where the engine can keep them).
    fn break_gathering(
        &self,
        host: &mut dyn Host,
        uuid: u128,
        pos: Pos,
        gathered: &mut Gathered,
    ) -> bool;
    /// Sets `gathered` down at `origin`, merged into as few stacks as fit;
    /// with `into_inventory` the player gets what fits first.
    fn drop_gathered(
        &self,
        host: &mut dyn Host,
        uuid: u128,
        origin: Pos,
        gathered: Gathered,
        into_inventory: bool,
    );
}

/// Horizon's own player break (`native_player_break_block`, and
/// `native_player_break_block_kept` with `native_pop_break_drops`).
#[cfg(feature = "player-break")]
pub(crate) struct EngineBreak;

#[cfg(feature = "player-break")]
impl PlayerBreak for EngineBreak {
    fn break_block(&self, host: &mut dyn Host, uuid: u128, pos: Pos, into_inventory: bool) -> bool {
        host.player_break_block(uuid, pos, into_inventory)
    }

    fn break_gathering(
        &self,
        host: &mut dyn Host,
        uuid: u128,
        pos: Pos,
        gathered: &mut Gathered,
    ) -> bool {
        let Some(kept) = host.player_break_block_kept(uuid, pos) else {
            return false;
        };
        gathered.stacks.extend(kept.stacks);
        gathered.experience = gathered.experience.saturating_add(kept.experience);
        true
    }

    fn drop_gathered(
        &self,
        host: &mut dyn Host,
        uuid: u128,
        origin: Pos,
        gathered: Gathered,
        into_inventory: bool,
    ) {
        let rest = if into_inventory {
            gathered
                .stacks
                .into_iter()
                .filter_map(|stack| host.give_stack(uuid, stack))
                .collect()
        } else {
            gathered.stacks
        };
        if (!rest.is_empty() || gathered.experience > 0)
            && !host.pop_break_drops(uuid, origin, rest, gathered.experience)
        {
            eprintln!("ultimine: the gathered drops could not be set down at {origin:?}");
        }
    }
}

/// Interim, until the engine helper is merged: `Level.destroyBlock` with the
/// loot rolled for the held tool (`native_machine_destroy_block`). Drops
/// and ore experience are right but land at each block; tool wear,
/// statistics and drops to the inventory are not, and nested
/// `destroy_block` hooks don't fire.
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

    fn break_gathering(
        &self,
        host: &mut dyn Host,
        uuid: u128,
        pos: Pos,
        _gathered: &mut Gathered,
    ) -> bool {
        self.break_block(host, uuid, pos, false)
    }

    fn drop_gathered(&self, _: &mut dyn Host, _: u128, _: Pos, _: Gathered, _: bool) {}
}

#[cfg(feature = "player-break")]
pub(crate) const BREAK: &dyn PlayerBreak = &EngineBreak;
#[cfg(not(feature = "player-break"))]
pub(crate) const BREAK: &dyn PlayerBreak = &InterimBreak;
