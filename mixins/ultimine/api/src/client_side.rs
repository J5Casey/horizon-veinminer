//! The client half's hooks: the activation key, scrolling through shapes,
//! telling the server, the outline preview and the HUD.
use crate::engine::client::Client;
use crate::engine::config::ClientPrefs;
use crate::engine::geom::Face;
use crate::engine::protocol::{self, MOD_ID};
use crate::engine::shape::ShapeId;
use crate::world::HostWorld;
use horizon_client::mod_api::{Corner, Corners, Host, HudBatch, NativeBlockRim};

/// The activation key's name on the settings page, and its default: the
/// grave accent, as Ultimine's.
pub const KEY: &str = "activate";
pub const DEFAULT_KEY: &str = "key.keyboard.grave.accent";
/// The setting (file) holding the player's own preferences.
pub const PREFS_KEY: &str = "client";
/// Horizon's budget of outlined blocks per mod and frame.
const RIM_BUDGET: usize = 256;

#[derive(Default)]
pub struct ClientSide {
    state: Option<Client>,
}

impl ClientSide {
    pub fn client(&mut self, host: &dyn Host) -> &mut Client {
        self.state.get_or_insert_with(|| {
            let prefs = host
                .setting(MOD_ID, PREFS_KEY)
                .map(|t| ClientPrefs::from_json(&t))
                .unwrap_or_default();
            Client::new(prefs)
        })
    }

    pub fn save(&mut self, host: &dyn Host) {
        let prefs = self.client(host).prefs.to_json();
        host.set_setting(MOD_ID, PREFS_KEY, &prefs);
    }

    /// Keys reach this on any screen: a press counts only while playing
    /// (typing a ` in chat doesn't activate), a release always.
    pub fn key(&mut self, host: &dyn Host, key: &str, pressed: bool) {
        if key == host.mod_key(MOD_ID, KEY, DEFAULT_KEY) && (!pressed || host.playing()) {
            self.client(host).key_down = pressed;
        }
    }

    /// Wheel steps while playing; true takes them from the hotbar.
    pub fn wheel(&mut self, host: &dyn Host, steps: f32) -> bool {
        let taken = self.client(host).scroll(steps);
        if taken {
            self.save(host);
        }
        taken
    }

    pub fn server_message(&mut self, host: &dyn Host, channel: &str, data: &[u8]) {
        if channel == protocol::RULES
            && let Some(rules) = protocol::decode_rules(data)
        {
            self.client(host).rules = Some(rules);
        }
    }

    /// The window's tick: activation, messages to the server and the preview.
    pub fn tick(&mut self, host: &mut dyn Host) {
        let in_world = host.player();
        let client = self.client(host);
        let Some(player) = in_world else {
            client.disconnected();
            return;
        };
        client.sneaking = host.sneaking();
        if let Some(state) = client.outgoing() {
            // An error is a server without mod messages: nothing to do.
            let _ = host.net_send_to_server(protocol::STATE, &protocol::encode_state(&state));
        }
        let target = if host.playing() {
            host.view_hit().map(|(_, hit)| {
                let pos = [hit.block_x, hit.block_y, hit.block_z];
                (pos, Face::from_hit(pos, [hit.x, hit.y, hit.z]))
            })
        } else {
            None
        };
        let facing = Face::from_yaw(player.yaw);
        client.update_preview(&HostWorld(host), target, facing);
    }

    pub fn visuals(&mut self, host: &dyn Host, rims: &mut Vec<NativeBlockRim>) {
        let client = self.client(host);
        if !client.active() || !client.prefs.outline || client.preview.len() <= 1 {
            return;
        }
        let limit = client.prefs.outline_limit.min(RIM_BUDGET);
        rims.extend(
            client
                .preview
                .iter()
                .take(limit)
                .map(|&pos| NativeBlockRim {
                    pos,
                    argb: 0xC0FF_FFFF,
                    width: 2.0,
                }),
        );
    }

    pub fn hud(&mut self, host: &dyn Host, batch: &mut HudBatch, corners: &mut Corners) {
        let client = self.client(host);
        if !client.active() || !client.prefs.hud {
            return;
        }
        let rules = client.rules();
        let current = client.shape();
        let count = client.preview.len();
        let title = if count > 0 {
            format!("Ultimine: {} ({count})", current.label())
        } else {
            format!("Ultimine: {}", current.label())
        };
        // Ultimine's menu: the shapes around the current one, two each way.
        let order: Vec<ShapeId> = ShapeId::ALL
            .into_iter()
            .filter(|s| rules.allows(*s))
            .collect();
        let at = order.iter().position(|s| *s == current).unwrap_or(0) as i32;
        let mut lines = vec![(title, 0xFF_FFFF)];
        if order.len() > 1 {
            let span = (order.len() as i32 - 1).min(4) / 2;
            for d in -span..=span {
                let shape = order[(at + d).rem_euclid(order.len() as i32) as usize];
                let (mark, rgb) = if d == 0 {
                    ("> ", 0xFF_FF55)
                } else {
                    ("  ", 0xAA_AAAA)
                };
                lines.push((format!("{mark}{}", shape.label()), rgb));
            }
        }
        if client.rules.is_none() {
            lines.push(("(server without Ultimine)".into(), 0xFF_5555));
        }
        let width = lines
            .iter()
            .map(|(t, _)| host.text_width(t))
            .fold(0.0, f32::max)
            + 6.0;
        let height = lines.len() as f32 * 10.0 + 4.0;
        let (x, y) = corners.place(Corner::TopLeft, width, height);
        host.fill(batch, [x, y, width, height], 0x7800_0000);
        for (i, (text, rgb)) in lines.iter().enumerate() {
            host.text(
                batch,
                text,
                [x + 3.0, y + 3.0 + i as f32 * 10.0],
                *rgb,
                true,
            );
        }
    }
}
