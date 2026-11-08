//! Horizon's player break fires `destroy_block` again for each block Ultimine
//! breaks. The guard makes those breaks pass through instead of starting
//! another Ultimine use.
use std::collections::BTreeMap;
use ultimine_engine::config::Rules;
use ultimine_engine::geom::{Face, Pos};
use ultimine_engine::matcher::NoTags;
use ultimine_engine::reentry::{Guard, active};
use ultimine_engine::select::{Request, World, select};
use ultimine_engine::server::{Breaker, execute};
use ultimine_engine::shape::ShapeId;

/// A vein whose breaks call back into the server's `destroy_block`, as the
/// engine's `player_break_block` does.
struct Vein {
    blocks: BTreeMap<Pos, String>,
    /// What the nested `destroy_block` did: let the break through, or start
    /// Ultimine again.
    passed: u32,
    restarted: u32,
}

impl Vein {
    /// The glue's `destroy_block`: Ultimine's own breaks pass through.
    fn destroy_block_hook(&mut self) {
        if active() {
            self.passed += 1;
        } else {
            self.restarted += 1;
        }
    }
}

impl World for Vein {
    fn block(&self, pos: Pos) -> Option<String> {
        Some(
            self.blocks
                .get(&pos)
                .cloned()
                .unwrap_or_else(|| "minecraft:air".into()),
        )
    }
}

impl Breaker for Vein {
    fn block(&self, pos: Pos) -> Option<String> {
        World::block(self, pos)
    }
    fn held(&self) -> Option<String> {
        Some("minecraft:iron_pickaxe".into())
    }
    fn food(&self) -> Option<i32> {
        Some(20)
    }
    fn break_block(&mut self, pos: Pos) -> bool {
        self.destroy_block_hook();
        self.blocks.remove(&pos).is_some()
    }
    fn exhaust(&mut self, _amount: f32) {}
}

#[test]
fn ultimines_own_breaks_never_start_it_again() {
    let blocks = (0..18)
        .map(|i| ([i % 6, i / 6, 0], "minecraft:iron_ore".to_owned()))
        .collect();
    let mut vein = Vein {
        blocks,
        passed: 0,
        restarted: 0,
    };
    let rules = Rules::default();
    let request = Request {
        origin: [0, 0, 0],
        face: Face::Up,
        facing: Face::North,
        shape: ShapeId::Shapeless,
    };
    let selection = select(&vein, &rules, &NoTags, &request);
    assert_eq!(selection.positions.len(), 18);
    let guard = Guard::start().expect("no guard yet");
    let report = execute(&mut vein, &selection, &rules, false);
    drop(guard);
    assert_eq!(report.broken, 18);
    assert_eq!((vein.passed, vein.restarted), (18, 0));
    // Without the guard a break would have started Ultimine again.
    assert!(!active());
    vein.destroy_block_hook();
    assert_eq!(vein.restarted, 1);
}

#[test]
fn the_guard_nests_once_and_ends_on_drop_or_unwind() {
    assert!(!active());
    let guard = Guard::start().unwrap();
    assert!(active());
    assert!(Guard::start().is_none());
    drop(guard);
    assert!(!active());
    let unwound = std::panic::catch_unwind(|| {
        let _guard = Guard::start().unwrap();
        panic!("a break went wrong");
    });
    assert!(unwound.is_err());
    assert!(!active());
}
