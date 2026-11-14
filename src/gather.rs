//! A vein's drops and experience, kept while Ultimine breaks it. Each of its
//! breaks (`PlayerRef::destroy_block`) fires `block_drops` with the player as
//! the breaker, inside the break and so inside the server half's lock: the
//! hook takes the item list and the experience into this state instead,
//! which has its own lock. Nothing spawns at the vein's other blocks; the
//! use sets it all down at the origin (`world::set_down`). What else a break
//! sets loose (a torch losing its support, a container's contents) is not
//! the block's own drop and still drops where it was.
use crate::engine::geom::Pos;
use mod_api::{BlockDropsEvent, ItemStack};
use std::collections::BTreeSet;
use std::sync::Mutex;

/// What one use's breaks set loose from the blocks themselves.
#[derive(Default)]
pub struct Kept {
    pub stacks: Vec<ItemStack>,
    pub experience: u32,
}

struct Gathering {
    player: u128,
    /// The blocks broken so far, whose drops are ours.
    expected: BTreeSet<Pos>,
    kept: Kept,
}

static GATHERING: Mutex<Option<Gathering>> = Mutex::new(None);

fn with<T>(f: impl FnOnce(&mut Option<Gathering>) -> T) -> T {
    let mut guard = GATHERING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut guard)
}

/// A use by `player` starts: their breaks' drops are kept from now on.
pub fn start(player: u128) {
    with(|g| {
        *g = Some(Gathering {
            player,
            expected: BTreeSet::new(),
            kept: Kept::default(),
        });
    });
}

/// The use is about to break `pos`.
pub fn expect(pos: Pos) {
    with(|g| {
        if let Some(g) = g {
            g.expected.insert(pos);
        }
    });
}

/// What the use kept; it ends the gathering.
pub fn take() -> Kept {
    with(|g| g.take().map(|g| g.kept).unwrap_or_default())
}

/// `block_drops`: a break of the running use's player at a block it broke
/// gives its drops and experience to the use.
pub fn keep(event: &mut BlockDropsEvent<'_>) {
    let pos = [event.pos.x, event.pos.y, event.pos.z];
    with(|g| {
        let Some(g) = g else {
            return;
        };
        if event.breaker != Some(g.player) || !g.expected.contains(&pos) {
            return;
        }
        g.kept
            .stacks
            .extend(event.drops.drain(..).map(|drop| drop.stack));
        g.kept.experience = g
            .kept
            .experience
            .saturating_add(std::mem::take(&mut event.experience));
    });
}
