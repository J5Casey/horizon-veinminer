//! Ultimine for Horizon: an FTB Ultimine-style vein miner as a native mod on
//! the mod API (`horizon-mod-api`, lib `mod_api`).
//!
//! One instance serves the window's game and the server's (single player
//! runs both), so the two halves keep their state apart behind locks. The
//! server hooks the game reaches again from inside Ultimine's own breaks
//! (`break_block`, `left_click_block`, `right_click_block`) return before
//! taking the lock while those breaks run; `block_drops`, which those breaks
//! fire, keeps the vein's drops in `gather`, under a lock of its own.
#![forbid(unsafe_code)]

mod client_side;
pub mod engine;
mod gather;
mod server_side;
mod world;

use client_side::ClientSide;
use engine::page::{self, Kind};
use engine::protocol;
use mod_api::{
    BlockDropsEvent, BlockHighlightEvent, BreakBlockEvent, ClientHost, ClientTickPostEvent, Flow,
    GuiLayer, GuiLayerDrawEvent, GuiLayerPlacement, HandlerThread, Hooks, HostSide,
    IPayloadContext, IPayloadHandler, KeyRegistration, LeftClickBlockEvent, LevelHost,
    MouseScrollingEvent, PayloadBytes, PayloadDeclaration, PayloadDirection, PayloadId,
    PayloadProtocol, PlayerLoggedOutEvent, RegisterGuiLayersEvent, Registrar, RegistrationContext,
    RightClickBlockEvent, ServerHost, ServerTickEvent, SettingChangedEvent, SettingSpec,
    VisualsEvent,
};
use server_side::ServerSide;
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Default)]
struct Ultimine {
    client: Arc<Mutex<ClientSide>>,
    server: Arc<Mutex<ServerSide>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A client's state, on the server.
struct StateHandler(Arc<Mutex<ServerSide>>);

impl IPayloadHandler for StateHandler {
    fn handle(
        &self,
        payload: PayloadBytes,
        context: &mut dyn IPayloadContext,
    ) -> Result<(), String> {
        let player = context.player().map_err(|e| format!("{e:?}"))?;
        let host = context.server_host().map_err(|e| format!("{e:?}"))?;
        lock(&self.0).client_message(host, player.uuid(), &payload.0);
        Ok(())
    }
}

/// The server's rules, on the client.
struct RulesHandler(Arc<Mutex<ClientSide>>);

impl IPayloadHandler for RulesHandler {
    fn handle(
        &self,
        payload: PayloadBytes,
        context: &mut dyn IPayloadContext,
    ) -> Result<(), String> {
        let host = context.client_host().map_err(|e| format!("{e:?}"))?;
        lock(&self.0).server_message(host, &payload.0);
        Ok(())
    }
}

fn declaration(id: &str, direction: PayloadDirection) -> PayloadDeclaration {
    // Optional: a server or client without Ultimine still connects.
    PayloadDeclaration::new(
        PayloadId(id.into()),
        protocol::VERSION.to_string(),
        PayloadProtocol::Play,
        direction,
        true,
        HandlerThread::Main,
    )
}

#[mod_api::horizon_mod]
impl Hooks for Ultimine {
    fn register(&self, r: &mut dyn Registrar, _cx: &RegistrationContext) {
        for setting in page::settings() {
            let spec = match setting.kind {
                Kind::Toggle(default) => SettingSpec::toggle(setting.key, setting.label, default),
                Kind::Choice(options, default) => {
                    SettingSpec::choice(setting.key, setting.label, options, default)
                }
            };
            if let Err(e) = r.register_setting(spec) {
                r.registration_refusal(format!("a setting: {e}"));
            }
        }
        r.register_key_mapping(
            KeyRegistration::new(client_side::KEY, client_side::DEFAULT_KEY)
                .label("key.ultimine.activate"),
        );
        r.add_payload(
            declaration(protocol::STATE, PayloadDirection::ToServer),
            Some(Arc::new(StateHandler(Arc::clone(&self.server)))),
            None,
        );
        r.add_payload(
            declaration(protocol::RULES, PayloadDirection::ToClient),
            None,
            Some(Arc::new(RulesHandler(Arc::clone(&self.client)))),
        );
    }

    fn register_gui_layers(
        &self,
        _cx: &RegistrationContext,
        event: &mut RegisterGuiLayersEvent<'_>,
    ) {
        event.layers.push(GuiLayer {
            id: client_side::LAYER.into(),
            placement: GuiLayerPlacement::AboveAll,
        });
    }

    fn setting_changed(&self, host: &mut dyn ClientHost, _event: &mut SettingChangedEvent<'_>) {
        lock(&self.client).settings_changed(host);
    }

    fn mouse_scrolling(&self, host: &mut dyn ClientHost, event: &mut MouseScrollingEvent) -> Flow {
        if lock(&self.client).wheel(host, event.scroll[1]) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    fn client_tick_post(&self, host: &mut dyn ClientHost, _event: &mut ClientTickPostEvent) {
        lock(&self.client).tick(host);
    }

    fn server_tick(&self, host: &mut dyn ServerHost, event: &mut ServerTickEvent<'_>) {
        if matches!(event.phase, mod_api::ServerTickPhase::ServerTickEnd { .. }) {
            lock(&self.server).tick(host);
        }
    }

    fn visuals(&self, host: &dyn ClientHost, event: &mut VisualsEvent<'_>) {
        lock(&self.client).visuals(host, event.rims);
    }

    fn block_highlight(&self, host: &dyn ClientHost, event: &mut BlockHighlightEvent) -> Flow {
        if lock(&self.client).replaces_highlight(host, event.pos) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    fn gui_layer_draw(&self, host: &dyn ClientHost, event: &mut GuiLayerDrawEvent<'_>) {
        if event.layer == client_side::LAYER {
            lock(&self.client).hud(host, event.draw);
        }
    }

    fn player_logged_out(&self, _host: &mut dyn ServerHost, event: &mut PlayerLoggedOutEvent<'_>) {
        lock(&self.server).logged_out(event.player);
    }

    fn left_click_block(&self, host: &mut dyn ServerHost, event: &mut LeftClickBlockEvent) -> Flow {
        if !server_side::breaking() {
            lock(&self.server).left_click(host, event.pos, event.face);
        }
        Flow::Continue
    }

    fn break_block(&self, host: &mut dyn ServerHost, event: &mut BreakBlockEvent<'_>) -> Flow {
        // Ultimine's own breaks come back here; they are left alone, and the
        // lock is never taken while one runs.
        if server_side::breaking() {
            return Flow::Continue;
        }
        if lock(&self.server).break_block(host, event.pos) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    fn block_drops(&self, _level: &dyn LevelHost, event: &mut BlockDropsEvent<'_>) -> Flow {
        gather::keep(event);
        Flow::Continue
    }

    fn right_click_block(&self, host: HostSide, event: &mut RightClickBlockEvent<'_>) -> Flow {
        let HostSide::Server(host) = host else {
            return Flow::Continue;
        };
        if server_side::breaking() {
            return Flow::Continue;
        }
        let (pos, face) = (event.click.pos, event.click.hit.face);
        if lock(&self.server).right_click(host, pos, face) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }
}
