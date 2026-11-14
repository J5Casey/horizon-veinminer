//! The server half's hooks: player state from clients, the rules, and the
//! authoritative selection and breaking on `break_block`.
use crate::engine::config::Rules;
use crate::engine::geom::{Face, Pos};
use crate::engine::page::{self, Value};
use crate::engine::protocol::{self, MOD_ID};
use crate::engine::rightclick;
use crate::engine::select::{Request, select};
use crate::engine::server::{Actor, GameMode, Server, execute};
use crate::gather;
use crate::world::{HostBreaker, HostFarmer, HostTags, HostWorld, held};
use mod_api::{GameMode as Mode, Host, LevelRef, PayloadBytes, PayloadId, PlayerRef, ServerHost};
use std::collections::BTreeMap;

/// Ticks between reads of the rules' settings.
const RELOAD_EVERY: u64 = 40;

/// Ultimine's own breaks reach `break_block` again (each one is the
/// player's break): see `engine::reentry`.
use crate::engine::reentry::Guard as Breaking;

pub fn breaking() -> bool {
    crate::engine::reentry::active()
}

/// The rules as this game's settings hold them (the page edits these).
pub fn read_rules(host: &dyn Host) -> Rules {
    page::read(&|key| setting(host, key)).1
}

/// A setting's value as kept (`true`/`false` for a toggle, the option's
/// index for a choice); None for one the player never changed. The typed
/// reads are the client's only, and a server reads the rules too.
pub fn setting(host: &dyn Host, key: &str) -> Option<Value> {
    let raw = host.setting(MOD_ID, key)?;
    match raw.trim() {
        "true" => Some(Value::Toggle(true)),
        "false" => Some(Value::Toggle(false)),
        n => n.parse().ok().map(Value::Choice),
    }
}

#[derive(Default)]
pub struct ServerSide {
    engine: Server,
    rules: Option<Rules>,
    now: u64,
    /// The last right click handled per player, so the off hand's turn of
    /// the same click doesn't act again.
    last_click: BTreeMap<u128, (u64, Pos)>,
}

fn send_rules(host: &mut dyn ServerHost, uuid: u128, rules: &Rules) {
    if let Some(player) = host.player(uuid) {
        // An error is a client without Ultimine: nothing to do.
        let _ = player.send_payload(
            host,
            &PayloadId(protocol::RULES.into()),
            PayloadBytes(protocol::encode_rules(rules)),
        );
    }
}

impl ServerSide {
    fn rules(&mut self, host: &dyn Host) -> Rules {
        self.rules.get_or_insert_with(|| read_rules(host)).clone()
    }

    pub fn tick(&mut self, host: &mut dyn ServerHost) {
        self.now += 1;
        if !self.now.is_multiple_of(RELOAD_EVERY) {
            return;
        }
        let rules = read_rules(host);
        eprintln!(
            "ultimine-debug: tick {} drops_to_inventory {} raw {:?}",
            self.now,
            rules.drops_to_inventory,
            host.setting(MOD_ID, "drops_to_inventory")
        );
        if self.rules.as_ref() == Some(&rules) {
            return;
        }
        self.rules = Some(rules.clone());
        for uuid in self.engine.rules_changed() {
            send_rules(host, uuid, &rules);
        }
    }

    pub fn client_message(&mut self, host: &mut dyn ServerHost, uuid: u128, data: &[u8]) {
        let Some(state) = protocol::decode_state(data) else {
            return;
        };
        let rules = self.rules(host);
        if self.engine.client_state(uuid, &state, &rules) {
            send_rules(host, uuid, &rules);
        }
    }

    pub fn left_click(&mut self, host: &dyn ServerHost, pos: Pos, face: [i32; 3]) {
        if let (Some(player), Some(face)) =
            (host.players().acting_player(), Face::from_normal(face))
        {
            self.engine.dig_started(player.uuid(), pos, face);
        }
    }

    pub fn logged_out(&mut self, uuid: u128) {
        self.engine.forget(uuid);
        self.last_click.remove(&uuid);
    }

    /// The acting player's facts, and their Ultimine request for `origin`.
    fn prepare(
        &mut self,
        host: &dyn ServerHost,
        origin: Pos,
    ) -> Option<(Rules, Actor, PlayerRef, LevelRef, Request)> {
        if breaking() {
            return None;
        }
        let player = host.players().acting_player()?;
        let level = player.level(host)?;
        let rules = self.rules(host);
        let [yaw, pitch] = player.entity().rotation(host).unwrap_or_default();
        let actor = Actor {
            uuid: player.uuid(),
            mode: match player.game_mode(host) {
                Some(Mode::Creative) => GameMode::Creative,
                Some(Mode::Adventure) => GameMode::Adventure,
                Some(Mode::Spectator) => GameMode::Spectator,
                _ => GameMode::Survival,
            },
            yaw,
            pitch,
            food: player.food(host).map(|f| f.level),
            held: held(host, player),
        };
        let request = self.engine.request(&actor, origin, self.now, &rules).ok()?;
        Some((rules, actor, player, level, request))
    }

    /// The acting player breaks `origin`. When Ultimine is active and finds
    /// more than the one block, it breaks them all (the origin first) as the
    /// player, keeps their drops and experience, sets them down at the
    /// origin and returns true: the original break is then cancelled.
    pub fn break_block(&mut self, host: &mut dyn ServerHost, origin: Pos) -> bool {
        let Some((rules, actor, player, level, request)) = self.prepare(host, origin) else {
            return false;
        };
        let world = HostWorld { host, level };
        let selection = select(&world, &rules, &HostTags(host), &request);
        if selection.len() <= 1 {
            return false;
        }
        let Some(guard) = Breaking::start() else {
            return false;
        };
        let creative = actor.mode == GameMode::Creative;
        gather::start(actor.uuid);
        let mut breaker = HostBreaker {
            host,
            player,
            level,
            into_inventory: rules.drops_to_inventory,
        };
        let report = execute(&mut breaker, &selection, &rules, creative);
        drop(guard);
        if !creative && report.broken > 0 {
            self.engine.used(actor.uuid, self.now, &rules);
        }
        true
    }

    /// A right click on `pos` with Ultimine active: the right-click action
    /// on every selected block it suits. True when it did anything (the
    /// click is then taken).
    pub fn right_click(&mut self, host: &mut dyn ServerHost, pos: Pos, face: [i32; 3]) -> bool {
        if let Some(player) = host.players().acting_player()
            && self.last_click.get(&player.uuid()) == Some(&(self.now, pos))
        {
            return true;
        }
        let Some((rules, actor, player, level, mut request)) = self.prepare(host, pos) else {
            return false;
        };
        if let Some(face) = Face::from_normal(face) {
            request.face = face;
        }
        let world = HostWorld { host, level };
        let selection = select(&world, &rules, &HostTags(host), &request);
        if selection.is_empty() {
            return false;
        }
        let Some(guard) = Breaking::start() else {
            return false;
        };
        let creative = actor.mode == GameMode::Creative;
        let tool = rightclick::tool_kind(actor.held.as_deref());
        let mut farmer = HostFarmer {
            host,
            player,
            level,
            into_inventory: rules.drops_to_inventory,
            creative,
        };
        let done = rightclick::apply(&mut farmer, &selection, &rules, tool);
        drop(guard);
        if done == 0 {
            return false;
        }
        self.last_click.insert(actor.uuid, (self.now, pos));
        if !creative {
            self.engine.used(actor.uuid, self.now, &rules);
        }
        true
    }
}
