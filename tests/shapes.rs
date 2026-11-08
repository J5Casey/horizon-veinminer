//! Shape geometry, checked against what Ultimine does in game for each shape.
use std::collections::{BTreeMap, BTreeSet};
use ultimine_engine::config::Rules;
use ultimine_engine::geom::{Face, Pos};
use ultimine_engine::matcher::NoTags;
use ultimine_engine::select::{Request, World, select};
use ultimine_engine::shape::ShapeId;

struct Blocks(BTreeMap<Pos, String>);

impl World for Blocks {
    fn block(&self, pos: Pos) -> Option<String> {
        Some(
            self.0
                .get(&pos)
                .cloned()
                .unwrap_or_else(|| "minecraft:air".into()),
        )
    }
}

/// A solid cube of stone, `r` blocks around the origin.
fn stone(r: i32) -> Blocks {
    let mut map = BTreeMap::new();
    for x in -r..=r {
        for y in -r..=r {
            for z in -r..=r {
                map.insert([x, y, z], "minecraft:stone".to_string());
            }
        }
    }
    Blocks(map)
}

fn run(world: &Blocks, shape: ShapeId, face: Face, facing: Face, max: usize) -> Vec<Pos> {
    let rules = Rules {
        max_blocks: max,
        ..Rules::default()
    };
    let req = Request {
        origin: [0, 0, 0],
        face,
        facing,
        shape,
    };
    select(world, &rules, &NoTags, &req).positions
}

fn set(v: &[Pos]) -> BTreeSet<Pos> {
    v.iter().copied().collect()
}

#[test]
fn small_tunnel_is_a_line_into_the_face_hit() {
    let w = stone(20);
    let line = run(&w, ShapeId::SmallTunnel, Face::South, Face::North, 64);
    // Hitting the south face, the player stands south: the tunnel runs north.
    assert_eq!(line.len(), 21);
    assert_eq!(line[..3], [[0, 0, 0], [0, 0, -1], [0, 0, -2]]);
    let down = run(&w, ShapeId::SmallTunnel, Face::Up, Face::North, 5);
    assert_eq!(
        down,
        vec![[0, 0, 0], [0, -1, 0], [0, -2, 0], [0, -3, 0], [0, -4, 0]]
    );
}

#[test]
fn small_tunnel_stops_at_the_first_gap() {
    let mut w = stone(5);
    w.0.remove(&[3, 0, 0]);
    let line = run(&w, ShapeId::SmallTunnel, Face::West, Face::East, 64);
    assert_eq!(line, vec![[0, 0, 0], [1, 0, 0], [2, 0, 0]]);
}

#[test]
fn small_square_is_one_3x3_across_the_face() {
    let w = stone(5);
    let square = run(&w, ShapeId::SmallSquare, Face::North, Face::South, 64);
    assert_eq!(square.len(), 9);
    assert_eq!(square[0], [0, 0, 0]);
    let expect: BTreeSet<Pos> = (-1..=1)
        .flat_map(|x| (-1..=1).map(move |y| [x, y, 0]))
        .collect();
    assert_eq!(set(&square), expect);
    // On a floor the square lies flat.
    let floor = run(&w, ShapeId::SmallSquare, Face::Up, Face::South, 64);
    assert!(floor.iter().all(|p| p[1] == 0));
    assert_eq!(floor.len(), 9);
}

#[test]
fn large_tunnel_is_3x3_layers_into_the_face_until_a_layer_is_empty() {
    let mut w = stone(10);
    // Clear the layers at x = 4 entirely: the tunnel ends there.
    for y in -1..=1 {
        for z in -1..=1 {
            w.0.remove(&[4, y, z]);
        }
    }
    let tunnel = run(&w, ShapeId::LargeTunnel, Face::West, Face::East, 1000);
    assert_eq!(tunnel.len(), 4 * 9);
    assert!(tunnel.iter().all(|p| (0..4).contains(&p[0])));
    assert_eq!(tunnel[0], [0, 0, 0]);
    // Layer by layer, in order.
    for (i, p) in tunnel.iter().enumerate() {
        assert_eq!(p[0], (i / 9) as i32, "{p:?} at {i}");
    }
}

#[test]
fn large_tunnel_respects_the_block_limit_mid_layer() {
    let w = stone(10);
    let tunnel = run(&w, ShapeId::LargeTunnel, Face::Up, Face::North, 64);
    assert_eq!(tunnel.len(), 64);
    assert_eq!(tunnel.iter().filter(|p| p[1] == -7).count(), 1);
}

#[test]
fn mining_tunnel_goes_down_diagonally_forward() {
    let w = stone(20);
    let t = run(&w, ShapeId::MiningTunnel, Face::South, Face::North, 4);
    assert_eq!(t, vec![[0, 0, 0], [0, -1, -1], [0, -2, -2], [0, -3, -3]]);
    // Hitting the top of a block, forward is the way the player looks.
    let t = run(&w, ShapeId::MiningTunnel, Face::Up, Face::East, 3);
    assert_eq!(t, vec![[0, 0, 0], [1, -1, 0], [2, -2, 0]]);
}

#[test]
fn escape_tunnel_goes_up_diagonally_forward() {
    let w = stone(20);
    let t = run(&w, ShapeId::EscapeTunnel, Face::East, Face::West, 3);
    assert_eq!(t, vec![[0, 0, 0], [-1, 1, 0], [-2, 2, 0]]);
    let t = run(&w, ShapeId::EscapeTunnel, Face::Down, Face::South, 3);
    assert_eq!(t, vec![[0, 0, 0], [0, 1, 1], [0, 2, 2]]);
}

#[test]
fn shapeless_follows_diagonal_veins_nearest_first() {
    let mut map = BTreeMap::new();
    for p in [[0, 0, 0], [1, 1, 1], [2, 2, 2], [2, 2, 3], [5, 5, 5]] {
        map.insert(p, "minecraft:iron_ore".to_string());
    }
    map.insert([1, 0, 0], "minecraft:deepslate_iron_ore".to_string());
    map.insert([0, 1, 0], "minecraft:gold_ore".to_string());
    let w = Blocks(map);
    let vein = run(&w, ShapeId::Shapeless, Face::Up, Face::North, 64);
    assert_eq!(
        vein,
        vec![[0, 0, 0], [1, 0, 0], [1, 1, 1], [2, 2, 2], [2, 2, 3]]
    );
}

#[test]
fn shapeless_is_bounded_deterministic_and_counts_the_origin() {
    let w = stone(8);
    let a = run(&w, ShapeId::Shapeless, Face::Up, Face::North, 7);
    let b = run(&w, ShapeId::Shapeless, Face::Up, Face::North, 7);
    assert_eq!(a, b);
    assert_eq!(a.len(), 7);
    assert_eq!(a[0], [0, 0, 0]);
}

#[test]
fn shapeless_crops_spread_through_the_plant_neighbourhood() {
    let mut map = BTreeMap::new();
    for x in [0, 2, 4] {
        map.insert([x, 0, 0], "minecraft:wheat".to_string());
    }
    map.insert([2, 0, 2], "minecraft:carrots".to_string());
    map.insert([2, 0, 3], "minecraft:oak_sapling".to_string());
    let w = Blocks(map);
    let field = run(&w, ShapeId::Shapeless, Face::Up, Face::North, 64);
    // Crops two apart are reached; a sapling is another kind of plant.
    assert_eq!(
        set(&field),
        set(&[[0, 0, 0], [2, 0, 0], [4, 0, 0], [2, 0, 2]])
    );
}

#[test]
fn shaped_mining_takes_any_block_but_spares_hard_blocks() {
    let mut w = stone(3);
    w.0.insert([1, 1, 0], "minecraft:dirt".into());
    w.0.insert([-1, -1, 0], "minecraft:obsidian".into());
    w.0.insert([0, 1, 0], "minecraft:water".into());
    let square = run(&w, ShapeId::SmallSquare, Face::South, Face::North, 64);
    assert!(square.contains(&[1, 1, 0]));
    assert!(!square.contains(&[-1, -1, 0]));
    assert!(!square.contains(&[0, 1, 0]));
    assert_eq!(square.len(), 7);
}

#[test]
fn unloaded_or_air_origins_select_nothing() {
    struct Unloaded;
    impl World for Unloaded {
        fn block(&self, _: Pos) -> Option<String> {
            None
        }
    }
    for shape in ShapeId::ALL {
        assert!(run(&Blocks(BTreeMap::new()), shape, Face::Up, Face::North, 64).is_empty());
        let req = Request {
            origin: [0, 0, 0],
            face: Face::Up,
            facing: Face::North,
            shape,
        };
        assert!(select(&Unloaded, &Rules::default(), &NoTags, &req).is_empty());
    }
}

#[test]
fn shapes_at_the_integer_limits_do_not_wrap() {
    let mut map = BTreeMap::new();
    let edge = [i32::MAX, 0, i32::MIN];
    map.insert(edge, "minecraft:stone".to_string());
    map.insert([i32::MIN, 0, i32::MIN], "minecraft:stone".to_string());
    let w = Blocks(map);
    for shape in ShapeId::ALL {
        let req = Request {
            origin: edge,
            face: Face::West,
            facing: Face::East,
            shape,
        };
        let got = select(&w, &Rules::default(), &NoTags, &req).positions;
        assert_eq!(got, vec![edge], "{shape:?}");
    }
}
