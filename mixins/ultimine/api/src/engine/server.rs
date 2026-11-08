//! The server half: each player's Ultimine state (from their client), the
//! decision whether a break becomes an Ultimine break, and the break loop.
//! Engine calls go through `Breaker`, so the loop is tested without a game.
use super::config::Rules;
use super::geom::{Face, Pos};
use super::protocol::ClientState;
use super::select::{Request, Selection};
use super::shape::ShapeId;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlayerState {
    pub active: bool,
    pub shape: ShapeId,
    /// The block the player last started digging, and the face they hit.
    pub dig: Option<(Pos, Face)>,
    pub cooldown_until: u64,
    /// The server's rules went to this player's client.
    pub rules_sent: bool,
}

/// Vanilla's game mode ids.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameMode {
    Survival,
    Creative,
    Adventure,
    Spectator,
}

impl GameMode {
    pub fn from_id(id: i32) -> GameMode {
        match id {
            1 => GameMode::Creative,
            2 => GameMode::Adventure,
            3 => GameMode::Spectator,
            _ => GameMode::Survival,
        }
    }
}

/// The acting player, as the server sees them at the break.
#[derive(Clone, Debug)]
pub struct Actor {
    pub uuid: u128,
    pub mode: GameMode,
    pub yaw: f32,
    pub pitch: f32,
    /// Food level 0..=20, when known.
    pub food: Option<i32>,
    /// The main hand's item id.
    pub held: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Inactive,
    GameMode,
    Cooldown,
    Hungry,
    NoTool,
}

/// Items that count as tools for `require_tool` (Ultimine: a damageable item
/// or one with a `tool` component).
pub fn is_tool(item: &str) -> bool {
    let path = item.rsplit(':').next().unwrap_or(item);
    ["_pickaxe", "_axe", "_shovel", "_hoe", "_sword", "_spear"]
        .iter()
        .any(|s| path.ends_with(s))
        || matches!(path, "shears" | "mace" | "trident")
}

#[derive(Default)]
pub struct Server {
    players: BTreeMap<u128, PlayerState>,
}

impl Server {
    pub fn player(&self, uuid: u128) -> Option<&PlayerState> {
        self.players.get(&uuid)
    }

    /// A state message from `uuid`'s client. True when the server should
    /// (re)send its rules: on first contact, or when the client asks.
    pub fn client_state(&mut self, uuid: u128, state: &ClientState, rules: &Rules) -> bool {
        let player = self.players.entry(uuid).or_default();
        player.active = state.active;
        player.shape = rules.effective_shape(state.shape);
        let send = !player.rules_sent || state.want_rules;
        player.rules_sent = true;
        send
    }

    /// The rules changed: every client needs them again.
    pub fn rules_changed(&mut self) -> Vec<u128> {
        self.players
            .iter_mut()
            .map(|(uuid, p)| {
                p.rules_sent = true;
                *uuid
            })
            .collect()
    }

    pub fn dig_started(&mut self, uuid: u128, pos: Pos, face: Face) {
        if let Some(p) = self.players.get_mut(&uuid) {
            p.dig = Some((pos, face));
        }
    }

    pub fn forget(&mut self, uuid: u128) {
        self.players.remove(&uuid);
    }

    /// Whether `actor` breaking `origin` at tick `now` is an Ultimine break,
    /// and with what request.
    pub fn request(
        &self,
        actor: &Actor,
        origin: Pos,
        now: u64,
        rules: &Rules,
    ) -> Result<Request, Refusal> {
        let player = self
            .players
            .get(&actor.uuid)
            .filter(|p| p.active)
            .ok_or(Refusal::Inactive)?;
        let creative = match actor.mode {
            GameMode::Survival => false,
            GameMode::Creative => true,
            GameMode::Adventure | GameMode::Spectator => return Err(Refusal::GameMode),
        };
        if !creative {
            if now < player.cooldown_until {
                return Err(Refusal::Cooldown);
            }
            if actor.food.is_some_and(|f| f <= 0) {
                return Err(Refusal::Hungry);
            }
        }
        if rules.require_tool && !actor.held.as_deref().is_some_and(is_tool) {
            return Err(Refusal::NoTool);
        }
        let facing = Face::from_yaw(actor.yaw);
        let face = match player.dig {
            Some((pos, face)) if pos == origin => face,
            _ => look_face(facing, actor.pitch),
        };
        Ok(Request {
            origin,
            face,
            facing,
            shape: rules.effective_shape(player.shape),
        })
    }

    /// A survival use finished: start the cooldown.
    pub fn used(&mut self, uuid: u128, now: u64, rules: &Rules) {
        if let Some(p) = self.players.get_mut(&uuid) {
            p.cooldown_until = now.saturating_add(u64::from(rules.cooldown_ticks));
        }
    }
}

/// The face a player most likely hit, from where they look, for a break
/// with no recorded dig start: the top when looking down, and so on.
pub fn look_face(facing: Face, pitch: f32) -> Face {
    if pitch > 45.0 {
        Face::Up
    } else if pitch < -45.0 {
        Face::Down
    } else {
        facing.opposite()
    }
}

/// The game calls the loop makes. `break_block` is the player's own break
/// (loot with their tool, durability, XP, statistics, hooks).
pub trait Breaker {
    fn block(&self, pos: Pos) -> Option<String>;
    /// The main hand's item id.
    fn held(&self) -> Option<String>;
    fn food(&self) -> Option<i32>;
    fn break_block(&mut self, pos: Pos) -> bool;
    fn exhaust(&mut self, amount: f32);
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub planned: usize,
    pub broken: usize,
    /// Blocks that changed since selection (an earlier break moved them).
    pub changed: usize,
    pub failed: usize,
    /// Stopped early: the tool broke, the player ran out of food, or a
    /// failure with `stop_on_break_fail`.
    pub stopped: bool,
}

/// Breaks `selection` in order, as Ultimine does.
pub fn execute(
    breaker: &mut dyn Breaker,
    selection: &Selection,
    rules: &Rules,
    creative: bool,
) -> Report {
    let snapshot: Vec<(Pos, Option<String>)> = selection
        .positions
        .iter()
        .map(|p| (*p, breaker.block(*p)))
        .collect();
    let tool = breaker.held();
    let mut report = Report {
        planned: snapshot.len(),
        ..Report::default()
    };
    for (pos, before) in snapshot {
        if before.is_none() || breaker.block(pos) != before {
            report.changed += 1;
            continue;
        }
        if !breaker.break_block(pos) {
            report.failed += 1;
            if rules.stop_on_break_fail {
                report.stopped = true;
                break;
            }
            continue;
        }
        report.broken += 1;
        if !creative {
            if rules.exhaustion() > 0.0 {
                breaker.exhaust(rules.exhaustion());
            }
            if breaker.food().is_some_and(|f| f <= 0) {
                report.stopped = true;
                break;
            }
        }
        if tool.is_some() && breaker.held() != tool {
            report.stopped = true;
            break;
        }
    }
    report
}
