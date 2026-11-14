//! The guard against Ultimine starting itself again. Its own breaks go
//! through Horizon's player break (`PlayerRef::destroy_block`), which fires
//! `break_block` again for each block, as any player's break does. While
//! a guard is held on this thread those hooks must let the break through
//! and never start another Ultimine use.
use std::cell::Cell;

thread_local! {
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
}

/// Held while Ultimine breaks or right-clicks blocks; dropping it (also on
/// unwinding) ends the guard.
#[derive(Debug)]
pub struct Guard(());

impl Guard {
    /// Starts the guard; None when one is already held on this thread.
    pub fn start() -> Option<Guard> {
        (!ACTIVE.with(Cell::get)).then(|| {
            ACTIVE.with(|a| a.set(true));
            Guard(())
        })
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        ACTIVE.with(|a| a.set(false));
    }
}

/// Whether Ultimine's own breaks are running on this thread: a hook that
/// hears one passes it through.
pub fn active() -> bool {
    ACTIVE.with(Cell::get)
}
