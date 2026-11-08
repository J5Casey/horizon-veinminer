//! Ultimine for Horizon: an FTB Ultimine-style vein miner as a native trait
//! mod. It uses only Horizon's catalog hooks and `Host` helpers; the game
//! links this crate and calls `hooks()`.
//!
//! One instance serves the window's game and the server's (single player
//! runs both), so the two halves keep their state apart behind locks. The
//! server hooks the game reaches again from inside Ultimine's own breaks
//! (`destroy_block`, `left_click_block`, `right_click_block`) return before
//! taking the lock while those breaks run.
#![forbid(unsafe_code)]

mod client_side;
pub mod engine;
mod player_break;
mod server_side;
mod world;

use client_side::ClientSide;
use engine::config::Rules;
use engine::page::{self, Changed};
use engine::protocol::MOD_ID;
use horizon_client::mod_api::{
    DestroyBlockEvent, Flow, Hooks, Host, HudEvent, KeyEvent, LeftClickBlockEvent, ModsEvent,
    NativeMod, NetFromClientEvent, NetFromServerEvent, PlayerLoggedOutEvent, RightClickBlockEvent,
    SettingsClickEvent, SettingsEvent, TickEvent, VisualsEvent, WheelEvent,
};
use server_side::ServerSide;
use std::sync::{Mutex, MutexGuard};

pub fn hooks() -> Box<dyn Hooks> {
    Box::new(Ultimine::default())
}

#[derive(Default)]
struct Ultimine {
    client: Mutex<ClientSide>,
    server: Mutex<ServerSide>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Ultimine {
    /// The rules as this game's settings hold them (the page edits these).
    fn page_rules(host: &dyn Host) -> Rules {
        server_side::parse_rules(host.setting(MOD_ID, server_side::RULES_KEY).as_deref())
            .unwrap_or_default()
    }
}

impl Hooks for Ultimine {
    fn mods(&self, _host: &mut dyn Host, event: &mut ModsEvent<'_>) {
        event.mods.push(NativeMod {
            id: MOD_ID.into(),
            title: format!("Ultimine {}", env!("CARGO_PKG_VERSION")),
            description: "Hold the Ultimine key and break or right-click a block to mine, \
                          till, strip or harvest everything connected to it. Scroll to change shape."
                .into(),
            keys: vec![[
                client_side::KEY.into(),
                "Ultimine (hold)".into(),
                client_side::DEFAULT_KEY.into(),
            ]],
        });
    }

    fn settings(&self, host: &mut dyn Host, event: &mut SettingsEvent<'_>) {
        if event.mod_id != MOD_ID {
            return;
        }
        let prefs = lock(&self.client).client(host).prefs.clone();
        event
            .buttons
            .extend(page::labels(&prefs, &Self::page_rules(host)));
    }

    fn settings_click(&self, host: &mut dyn Host, event: &mut SettingsClickEvent<'_>) {
        if event.mod_id != MOD_ID {
            return;
        }
        let mut client = lock(&self.client);
        let mut prefs = client.client(host).prefs.clone();
        let mut rules = Self::page_rules(host);
        match page::click(&mut prefs, &mut rules, event.button) {
            Changed::Nothing => {}
            Changed::Prefs => {
                client.client(host).prefs = prefs;
                client.save(host);
            }
            // A server hosted by this game reads the new rules within two
            // seconds and sends them to its players.
            Changed::Rules => host.set_setting(MOD_ID, server_side::RULES_KEY, &rules.to_json()),
        }
    }

    fn key(&self, host: &mut dyn Host, event: &mut KeyEvent<'_>) {
        lock(&self.client).key(host, event.key, event.pressed);
    }

    fn wheel(&self, host: &mut dyn Host, event: &mut WheelEvent) -> Flow {
        if lock(&self.client).wheel(host, event.steps) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    fn tick(&self, host: &mut dyn Host, _event: &mut TickEvent) {
        if host.window() {
            lock(&self.client).tick(host);
        } else {
            lock(&self.server).tick(host);
        }
    }

    fn visuals(&self, host: &dyn Host, event: &mut VisualsEvent<'_>) {
        lock(&self.client).visuals(host, event.rims);
    }

    fn hud(&self, host: &mut dyn Host, event: &mut HudEvent<'_>) {
        lock(&self.client).hud(host, event.batch, event.corners);
    }

    fn net_from_server(&self, host: &mut dyn Host, event: &mut NetFromServerEvent<'_>) {
        lock(&self.client).server_message(host, event.channel, event.data);
    }

    fn net_from_client(&self, host: &mut dyn Host, event: &mut NetFromClientEvent<'_>) {
        lock(&self.server).client_message(host, event.player, event.channel, event.data);
    }

    fn player_logged_out(&self, _host: &mut dyn Host, event: &mut PlayerLoggedOutEvent<'_>) {
        lock(&self.server).logged_out(event.player);
    }

    fn left_click_block(&self, host: &mut dyn Host, event: &mut LeftClickBlockEvent) -> Flow {
        if !server_side::breaking() {
            lock(&self.server).left_click(host, event.pos, event.face);
        }
        Flow::Continue
    }

    fn destroy_block(&self, host: &mut dyn Host, event: &mut DestroyBlockEvent) -> Flow {
        // Ultimine's own breaks come back here; they are left alone, and the
        // lock is never taken while one runs.
        if server_side::breaking() {
            return Flow::Continue;
        }
        if lock(&self.server).destroy_block(host, event.pos) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    fn right_click_block(&self, host: &mut dyn Host, event: &mut RightClickBlockEvent) -> Flow {
        if server_side::breaking() {
            return Flow::Continue;
        }
        if lock(&self.server).right_click(host, event.pos, event.face) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }
}
