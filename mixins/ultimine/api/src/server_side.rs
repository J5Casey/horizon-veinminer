//! The server half's hooks: player state from clients, the rules, and the
//! authoritative selection and breaking on `destroy_block`.
use crate::engine::config::Rules;
use crate::engine::geom::{Face, Pos};
use crate::engine::protocol::{self, MOD_ID};
use crate::engine::rightclick;
use crate::engine::select::select;
use crate::engine::server::{Actor, GameMode, Server, execute};
use crate::world::{HostBreaker, HostFarmer, HostTags, HostWorld};
use horizon_client::mod_api::Host;
use std::collections::BTreeMap;

/// The setting (file) holding the server's rules, as JSON.
pub const RULES_KEY: &str = "rules";
/// Ticks between checks of the rules file for changes.
const RELOAD_EVERY: u64 = 40;

/// Ultimine's own breaks reach `destroy_block` again (the engine's player
/// break fires it for each block): see `engine::reentry`.
use crate::engine::reentry::Guard as Breaking;

pub fn breaking() -> bool {
    crate::engine::reentry::active()
}

#[derive(Default)]
pub struct ServerSide {
    engine: Server,
    /// The rules file's text as last read, and what it gave: `None` when
    /// the file is invalid (Ultimine is then off: it fails closed).
    raw: Option<Option<String>>,
    rules: Option<Rules>,
    now: u64,
    /// The last right click handled per player, so the off hand's turn of
    /// the same click doesn't act again.
    last_click: BTreeMap<u128, (u64, Pos)>,
}

/// Reads the rules: the defaults when there is no file, an error when the
/// file is invalid.
pub fn parse_rules(raw: Option<&str>) -> Result<Rules, String> {
    match raw {
        None => Ok(Rules::default()),
        Some(text) => Rules::from_json(text),
    }
}

impl ServerSide {
    /// (Re)reads the rules file; true when the rules changed.
    fn reload(&mut self, host: &dyn Host) -> bool {
        let raw = host.setting(MOD_ID, RULES_KEY);
        if self.raw.as_ref() == Some(&raw) {
            return false;
        }
        self.rules = match parse_rules(raw.as_deref()) {
            Ok(rules) => Some(rules),
            Err(error) => {
                eprintln!("ultimine: invalid rules, Ultimine is off until they are fixed: {error}");
                None
            }
        };
        self.raw = Some(raw);
        true
    }

    fn rules(&mut self, host: &dyn Host) -> Option<Rules> {
        if self.raw.is_none() {
            self.reload(host);
        }
        self.rules.clone()
    }

    pub fn tick(&mut self, host: &mut dyn Host) {
        self.now += 1;
        if self.now.is_multiple_of(RELOAD_EVERY)
            && self.reload(host)
            && let Some(rules) = self.rules.clone()
        {
            let data = protocol::encode_rules(&rules);
            for uuid in self.engine.rules_changed() {
                let _ = host.net_send_to_player(uuid, protocol::RULES, &data);
            }
        }
    }

    pub fn client_message(&mut self, host: &mut dyn Host, uuid: u128, channel: &str, data: &[u8]) {
        if channel != protocol::STATE {
            return;
        }
        let Some(state) = protocol::decode_state(data) else {
            return;
        };
        let Some(rules) = self.rules(host) else {
            return;
        };
        if self.engine.client_state(uuid, &state, &rules) {
            let _ = host.net_send_to_player(uuid, protocol::RULES, &protocol::encode_rules(&rules));
        }
    }

    pub fn left_click(&mut self, host: &dyn Host, pos: Pos, face: [i32; 3]) {
        if let (Some(player), Some(face)) = (host.acting_player(), Face::from_normal(face)) {
            self.engine.dig_started(player.uuid, pos, face);
        }
    }

    pub fn logged_out(&mut self, uuid: u128) {
        self.engine.forget(uuid);
        self.last_click.remove(&uuid);
    }

    /// The acting player's facts, and their Ultimine request for `origin`.
    fn prepare(
        &mut self,
        host: &dyn Host,
        origin: Pos,
    ) -> Option<(Rules, Actor, String, crate::engine::select::Request)> {
        if breaking() {
            return None;
        }
        let player = host.acting_player()?;
        let rules = self.rules(host)?;
        let actor = Actor {
            uuid: player.uuid,
            mode: GameMode::from_id(player.game_mode),
            yaw: player.yaw,
            pitch: player.pitch,
            food: host.player_food(player.uuid).map(|(food, _, _)| food),
            held: host.held().map(|(item, _, _)| item),
        };
        let request = self.engine.request(&actor, origin, self.now, &rules).ok()?;
        Some((rules, actor, player.dimension, request))
    }

    /// The acting player breaks `origin`. When Ultimine is active and finds
    /// more than the one block, it breaks them all (the origin first) as the
    /// player and returns true: the original break is then cancelled.
    pub fn destroy_block(&mut self, host: &mut dyn Host, origin: Pos) -> bool {
        let Some((rules, actor, _, request)) = self.prepare(host, origin) else {
            return false;
        };
        let selection = select(&HostWorld(host), &rules, &HostTags(host), &request);
        if selection.len() <= 1 {
            return false;
        }
        let Some(guard) = Breaking::start() else {
            return false;
        };
        let creative = actor.mode == GameMode::Creative;
        let mut breaker = HostBreaker::new(host, actor.uuid, rules.drops_to_inventory);
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
    pub fn right_click(&mut self, host: &mut dyn Host, pos: Pos, face: [i32; 3]) -> bool {
        if let Some(player) = host.acting_player()
            && self.last_click.get(&player.uuid) == Some(&(self.now, pos))
        {
            return true;
        }
        let Some((rules, actor, dimension, mut request)) = self.prepare(host, pos) else {
            return false;
        };
        if let Some(face) = Face::from_normal(face) {
            request.face = face;
        }
        let selection = select(&HostWorld(host), &rules, &HostTags(host), &request);
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
            uuid: actor.uuid,
            dimension,
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
