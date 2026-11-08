//! The server's decisions and the break loop, the client's state machine
//! and right-click actions, driven through the engine's traits.
use std::collections::BTreeMap;
use ultimine_engine::client::Client;
use ultimine_engine::config::{Activation, ClientPrefs, Rules};
use ultimine_engine::geom::{Face, Pos};
use ultimine_engine::matcher::NoTags;
use ultimine_engine::protocol::{ClientState, VERSION};
use ultimine_engine::rightclick::{self, Action, Farmer, ToolKind};
use ultimine_engine::select::{Request, Selection, World, select};
use ultimine_engine::server::{Actor, Breaker, GameMode, Refusal, Server, execute};
use ultimine_engine::shape::ShapeId;

#[derive(Default)]
struct Game {
    blocks: BTreeMap<Pos, String>,
    held: Option<String>,
    /// Durability left; the tool breaks (the hand empties) at 0.
    durability: Option<u32>,
    food: i32,
    exhaustion: f32,
    calls: Vec<Pos>,
    fail: Vec<Pos>,
    /// Breaking the first key also turns the second into the third.
    side_effect: Option<(Pos, Pos, String)>,
    ages: BTreeMap<Pos, u8>,
    applied: Vec<(Pos, Action)>,
}

impl Game {
    fn ore(points: &[Pos]) -> Self {
        Game {
            blocks: points
                .iter()
                .map(|p| (*p, "minecraft:iron_ore".to_string()))
                .collect(),
            held: Some("minecraft:iron_pickaxe".into()),
            durability: Some(1000),
            food: 20,
            ..Game::default()
        }
    }
    fn select(&self, shape: ShapeId, rules: &Rules) -> Selection {
        let req = Request {
            origin: [0, 0, 0],
            face: Face::Up,
            facing: Face::North,
            shape,
        };
        select(self, rules, &NoTags, &req)
    }
}

impl World for Game {
    fn block(&self, pos: Pos) -> Option<String> {
        Some(
            self.blocks
                .get(&pos)
                .cloned()
                .unwrap_or_else(|| "minecraft:air".into()),
        )
    }
}

impl Breaker for Game {
    fn block(&self, pos: Pos) -> Option<String> {
        World::block(self, pos)
    }
    fn held(&self) -> Option<String> {
        self.held.clone()
    }
    fn food(&self) -> Option<i32> {
        Some(self.food)
    }
    fn break_block(&mut self, pos: Pos) -> bool {
        self.calls.push(pos);
        if self.fail.contains(&pos) || self.blocks.remove(&pos).is_none() {
            return false;
        }
        if let Some((when, other, name)) = self.side_effect.clone()
            && when == pos
        {
            self.blocks.insert(other, name);
        }
        if let Some(d) = self.durability.as_mut() {
            *d -= 1;
            if *d == 0 {
                self.held = None;
            }
        }
        true
    }
    fn exhaust(&mut self, amount: f32) {
        self.exhaustion += amount;
        if self.exhaustion >= 4.0 {
            self.exhaustion -= 4.0;
            self.food -= 1;
        }
    }
}

fn line(n: i32) -> Vec<Pos> {
    (0..n).map(|x| [x, 0, 0]).collect()
}

#[test]
fn breaks_every_selected_block_once_with_ultimines_hunger_cost() {
    let mut g = Game::ore(&line(5));
    let rules = Rules::default();
    let sel = g.select(ShapeId::Shapeless, &rules);
    let report = execute(&mut g, &sel, &rules, false);
    assert_eq!(
        (report.planned, report.broken, report.stopped),
        (5, 5, false)
    );
    assert_eq!(g.calls, line(5));
    assert!(g.blocks.is_empty());
    assert!((g.exhaustion - 0.5).abs() < 1e-5);
}

#[test]
fn creative_players_pay_nothing() {
    let mut g = Game::ore(&line(3));
    let rules = Rules::default();
    let sel = g.select(ShapeId::Shapeless, &rules);
    assert_eq!(execute(&mut g, &sel, &rules, true).broken, 3);
    assert_eq!(g.exhaustion, 0.0);
}

#[test]
fn a_breaking_tool_stops_the_chain() {
    let mut g = Game::ore(&line(6));
    g.durability = Some(2);
    let rules = Rules::default();
    let sel = g.select(ShapeId::Shapeless, &rules);
    let r = execute(&mut g, &sel, &rules, false);
    assert!(r.stopped);
    assert_eq!(r.broken, 2);
    assert_eq!(g.blocks.len(), 4);
}

#[test]
fn running_out_of_food_stops_the_chain() {
    let mut g = Game::ore(&line(10));
    g.food = 1;
    let rules = Rules {
        exhaustion_per_block: 400.0, // 2 exhaustion a block: a food point every 2
        ..Rules::default()
    };
    let sel = g.select(ShapeId::Shapeless, &rules);
    let r = execute(&mut g, &sel, &rules, false);
    assert!(r.stopped);
    assert_eq!(r.broken, 2);
}

#[test]
fn changed_blocks_are_skipped_and_failures_skip_or_stop() {
    let mut g = Game::ore(&line(4));
    g.side_effect = Some(([0, 0, 0], [2, 0, 0], "minecraft:gravel".into()));
    let rules = Rules::default();
    let sel = g.select(ShapeId::Shapeless, &rules);
    let r = execute(&mut g, &sel, &rules, false);
    assert_eq!((r.broken, r.changed), (3, 1));
    assert_eq!(g.blocks[&[2, 0, 0]], "minecraft:gravel");

    let mut g = Game::ore(&line(4));
    g.fail.push([1, 0, 0]);
    let sel = g.select(ShapeId::Shapeless, &rules);
    let r = execute(&mut g, &sel, &rules, false);
    assert_eq!((r.broken, r.failed, r.stopped), (3, 1, false));

    let mut g = Game::ore(&line(4));
    g.fail.push([1, 0, 0]);
    let stop = Rules {
        stop_on_break_fail: true,
        ..Rules::default()
    };
    let r = execute(&mut g, &sel, &stop, false);
    assert_eq!((r.broken, r.failed, r.stopped), (1, 1, true));
    assert_eq!(g.calls.len(), 2);
}

fn actor(mode: GameMode) -> Actor {
    Actor {
        uuid: 7,
        mode,
        yaw: 180.0,
        pitch: 0.0,
        food: Some(20),
        held: Some("minecraft:diamond_pickaxe".into()),
    }
}

fn state(active: bool, shape: ShapeId) -> ClientState {
    ClientState {
        v: VERSION,
        active,
        shape,
        want_rules: false,
    }
}

#[test]
fn the_server_mines_only_for_active_players_in_survival_or_creative() {
    let rules = Rules::default();
    let mut server = Server::default();
    let a = actor(GameMode::Survival);
    assert_eq!(
        server.request(&a, [0, 0, 0], 0, &rules),
        Err(Refusal::Inactive)
    );
    assert!(server.client_state(7, &state(true, ShapeId::LargeTunnel), &rules));
    assert!(!server.client_state(7, &state(true, ShapeId::LargeTunnel), &rules));
    let req = server.request(&a, [0, 0, 0], 0, &rules).unwrap();
    assert_eq!(req.shape, ShapeId::LargeTunnel);
    // Looking north (yaw 180) at a wall: the south face was hit.
    assert_eq!(req.face, Face::South);
    for mode in [GameMode::Adventure, GameMode::Spectator] {
        assert_eq!(
            server.request(&actor(mode), [0, 0, 0], 0, &rules),
            Err(Refusal::GameMode)
        );
    }
    let mut hungry = a.clone();
    hungry.food = Some(0);
    assert_eq!(
        server.request(&hungry, [0, 0, 0], 0, &rules),
        Err(Refusal::Hungry)
    );
    hungry.mode = GameMode::Creative;
    assert!(server.request(&hungry, [0, 0, 0], 0, &rules).is_ok());
    server.client_state(7, &state(false, ShapeId::LargeTunnel), &rules);
    assert_eq!(
        server.request(&a, [0, 0, 0], 0, &rules),
        Err(Refusal::Inactive)
    );
    server.forget(7);
    assert!(server.player(7).is_none());
}

#[test]
fn the_dig_face_wins_over_the_look_and_disabled_shapes_fall_back() {
    let rules = Rules {
        shapes: vec![ShapeId::Shapeless, ShapeId::SmallTunnel],
        ..Rules::default()
    };
    let mut server = Server::default();
    server.client_state(7, &state(true, ShapeId::LargeTunnel), &rules);
    server.dig_started(7, [0, 0, 0], Face::Up);
    let req = server
        .request(&actor(GameMode::Survival), [0, 0, 0], 0, &rules)
        .unwrap();
    assert_eq!((req.face, req.shape), (Face::Up, ShapeId::Shapeless));
    let req = server
        .request(&actor(GameMode::Survival), [5, 0, 0], 0, &rules)
        .unwrap();
    assert_eq!(req.face, Face::South);
}

#[test]
fn cooldowns_and_required_tools() {
    let rules = Rules {
        cooldown_ticks: 20,
        require_tool: true,
        ..Rules::default()
    };
    let mut server = Server::default();
    server.client_state(7, &state(true, ShapeId::Shapeless), &rules);
    let a = actor(GameMode::Survival);
    assert!(server.request(&a, [0, 0, 0], 100, &rules).is_ok());
    server.used(7, 100, &rules);
    assert_eq!(
        server.request(&a, [0, 0, 0], 119, &rules),
        Err(Refusal::Cooldown)
    );
    assert!(server.request(&a, [0, 0, 0], 120, &rules).is_ok());
    let mut bare = a.clone();
    bare.held = Some("minecraft:stick".into());
    assert_eq!(
        server.request(&bare, [0, 0, 0], 200, &rules),
        Err(Refusal::NoTool)
    );
    // Other players have their own cooldown.
    server.client_state(8, &state(true, ShapeId::Shapeless), &rules);
    let mut other = a.clone();
    other.uuid = 8;
    assert!(server.request(&other, [0, 0, 0], 101, &rules).is_ok());
}

#[test]
fn client_activation_scrolling_and_messages() {
    let mut c = Client::new(ClientPrefs::default());
    // First contact asks for the rules.
    let first = c.outgoing().unwrap();
    assert!(first.want_rules && !first.active);
    assert_eq!(c.outgoing(), None);
    assert!(!c.scroll(1.0), "inactive: the hotbar keeps the wheel");
    c.key_down = true;
    assert!(c.outgoing().unwrap().active);
    assert!(c.scroll(-1.0));
    assert_eq!(c.shape(), ShapeId::SmallTunnel);
    assert!(c.scroll(1.0));
    assert!(c.scroll(1.0));
    assert_eq!(c.shape(), ShapeId::EscapeTunnel);
    assert_eq!(c.outgoing().unwrap().shape, ShapeId::EscapeTunnel);
    // The server's rules narrow the shapes.
    c.rules = Some(Rules {
        shapes: vec![ShapeId::Shapeless, ShapeId::LargeTunnel],
        ..Rules::default()
    });
    assert_eq!(c.shape(), ShapeId::Shapeless);
    c.scroll(-1.0);
    assert_eq!(c.shape(), ShapeId::LargeTunnel);
    c.prefs.activation = Activation::Sneak;
    assert!(!c.active());
    c.sneaking = true;
    assert!(c.active());
}

#[test]
fn client_preview_matches_the_server_selection() {
    let g = Game::ore(&[[0, 0, 0], [1, 1, 0], [2, 2, 0]]);
    let mut c = Client::new(ClientPrefs::default());
    c.update_preview(&g, Some(([0, 0, 0], Face::Up)), Face::North);
    assert!(c.preview.is_empty(), "inactive: no preview");
    c.key_down = true;
    c.update_preview(&g, Some(([0, 0, 0], Face::Up)), Face::North);
    let server = g.select(ShapeId::Shapeless, &Rules::default());
    assert_eq!(c.preview, server.positions);
    c.update_preview(&g, None, Face::North);
    assert!(c.preview.is_empty());
}

impl Farmer for Game {
    fn block(&self, pos: Pos) -> Option<String> {
        World::block(self, pos)
    }
    fn air_above(&self, pos: Pos) -> bool {
        !self.blocks.contains_key(&[pos[0], pos[1] + 1, pos[2]])
    }
    fn age(&self, pos: Pos) -> Option<u8> {
        self.ages.get(&pos).copied()
    }
    fn apply(&mut self, pos: Pos, action: &Action) -> bool {
        self.applied.push((pos, action.clone()));
        true
    }
}

#[test]
fn right_click_harvests_only_ripe_crops_and_tills_open_dirt() {
    let mut g = Game::default();
    for x in 0..4 {
        g.blocks.insert([x, 0, 0], "minecraft:wheat".into());
        g.ages.insert([x, 0, 0], if x == 2 { 3 } else { 7 });
    }
    let rules = Rules::default();
    let sel = g.select(ShapeId::Shapeless, &rules);
    assert_eq!(sel.len(), 4);
    assert_eq!(rightclick::apply(&mut g, &sel, &rules, ToolKind::Other), 3);
    assert!(
        g.applied
            .iter()
            .all(|(p, a)| p[0] != 2 && matches!(a, Action::Harvest(_)))
    );

    let mut g = Game::default();
    for x in 0..3 {
        g.blocks.insert([x, 0, 0], "minecraft:grass_block".into());
    }
    g.blocks.insert([1, 1, 0], "minecraft:stone".into());
    let sel = g.select(ShapeId::Shapeless, &rules);
    assert_eq!(rightclick::apply(&mut g, &sel, &rules, ToolKind::Hoe), 2);
    assert_eq!(rightclick::apply(&mut g, &sel, &rules, ToolKind::Axe), 0);
    let off = Rules {
        right_click_hoe: false,
        ..Rules::default()
    };
    assert_eq!(rightclick::apply(&mut g, &sel, &off, ToolKind::Hoe), 0);
}

#[test]
fn axes_strip_logs() {
    let mut g = Game::default();
    for y in 0..3 {
        g.blocks.insert([0, y, 0], "minecraft:spruce_log".into());
    }
    let rules = Rules::default();
    let sel = g.select(ShapeId::Shapeless, &rules);
    assert_eq!(rightclick::apply(&mut g, &sel, &rules, ToolKind::Axe), 3);
    assert_eq!(
        g.applied[0].1,
        Action::Strip("minecraft:stripped_spruce_log".into())
    );
    assert_eq!(
        rightclick::tool_kind(Some("minecraft:golden_axe")),
        ToolKind::Axe
    );
    assert_eq!(
        rightclick::tool_kind(Some("minecraft:pickaxe_handle")),
        ToolKind::Other
    );
    assert_eq!(
        rightclick::tool_kind(Some("minecraft:diamond_pickaxe")),
        ToolKind::Other
    );
}

#[test]
fn tree_felling_takes_the_trunk_and_branches_but_not_the_neighbouring_tree() {
    let mut g = Game::default();
    for y in 0..6 {
        g.blocks.insert([0, y, 0], "minecraft:oak_log".into());
    }
    g.blocks.insert([1, 4, 0], "minecraft:oak_wood".into());
    g.blocks.insert([2, 5, 1], "minecraft:oak_log".into());
    g.blocks.insert([-1, 2, 0], "minecraft:oak_leaves".into());
    g.blocks.insert([0, 3, 1], "minecraft:birch_log".into());
    let sel = g.select(ShapeId::Shapeless, &Rules::default());
    assert_eq!(sel.len(), 8);
    assert!(!sel.positions.contains(&[0, 3, 1]));
    assert!(!sel.positions.contains(&[-1, 2, 0]));
}
