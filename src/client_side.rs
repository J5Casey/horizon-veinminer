//! The client half's hooks: the activation key, scrolling through shapes,
//! telling the server, the outline preview and the HUD overlay.
use crate::engine::client::Client;
use crate::engine::geom::{Face, Pos};
use crate::engine::page::{self, SHAPE};
use crate::engine::protocol::{self, MOD_ID};
use crate::engine::shape::ShapeId;
use crate::server_side::setting;
use crate::world::HostWorld;
use mod_api::{BlockRim, ClientHost, DrawContext, Host, PayloadBytes, PayloadId};

/// The activation key's id (Controls lists it as "Ultimine (hold)"), and its
/// default: the grave accent, as Ultimine's.
pub const KEY: &str = "activate";
pub const DEFAULT_KEY: &str = "key.keyboard.grave.accent";
/// The HUD overlay's GUI layer.
pub const LAYER: &str = "ultimine:overlay";
/// Horizon's budget of outlined blocks per mod and frame.
const RIM_BUDGET: usize = 256;

#[derive(Default)]
pub struct ClientSide {
    state: Option<Client>,
}

impl ClientSide {
    pub fn client(&mut self, host: &dyn Host) -> &mut Client {
        self.state
            .get_or_insert_with(|| Client::new(page::read(&|key| setting(host, key)).0))
    }

    /// The player changed a setting on the page: read the preferences again.
    pub fn settings_changed(&mut self, host: &dyn Host) {
        let prefs = page::read(&|key| setting(host, key)).0;
        self.client(host).prefs = prefs;
    }

    /// Wheel steps while playing; true takes them from the hotbar.
    pub fn wheel(&mut self, host: &dyn ClientHost, steps: f64) -> bool {
        let client = self.client(host);
        let taken = client.scroll(steps as f32);
        if taken {
            let shape = client.prefs.shape.index().to_string();
            host.set_setting(MOD_ID, SHAPE, &shape);
        }
        taken
    }

    pub fn server_message(&mut self, host: &dyn Host, data: &[u8]) {
        if let Some(rules) = protocol::decode_rules(data) {
            self.client(host).rules = Some(rules);
        }
    }

    /// The window's tick: activation, messages to the server and the preview.
    pub fn tick(&mut self, host: &mut dyn ClientHost) {
        let player = host.local_player();
        let level = player.and_then(|p| p.level(host));
        let (Some(player), Some(level)) = (player, level) else {
            self.client(host).disconnected();
            return;
        };
        // A press counts only while playing (typing a ` in chat doesn't
        // activate), a release always.
        let down = host.is_down(MOD_ID, KEY);
        let playing = host.playing();
        let sneaking = player.entity().is_shift_key_down(host).unwrap_or_default();
        let client = self.client(host);
        client.key_down = down && (playing || client.key_down);
        client.sneaking = sneaking;
        if let Some(state) = client.outgoing() {
            // An error is a server without Ultimine: nothing to do.
            let _ = host.send_payload_to_server(
                &PayloadId(protocol::STATE.into()),
                PayloadBytes(protocol::encode_state(&state)),
            );
        }
        let target = if playing {
            host.view_hit().map(|(_, hit)| {
                let pos = [hit.block_x, hit.block_y, hit.block_z];
                (pos, Face::from_hit(pos, [hit.x, hit.y, hit.z]))
            })
        } else {
            None
        };
        let yaw = player.entity().rotation(host).map_or(0.0, |[yaw, _]| yaw);
        let world = HostWorld { host, level };
        let client = self.client(host);
        client.update_preview(&world, target, Face::from_yaw(yaw));
    }

    /// The blocks the preview outlines this frame: none while inactive, with
    /// outlines off, or for a lone block (the vanilla outline is enough).
    fn outlined(&mut self, host: &dyn Host) -> &[Pos] {
        let client = self.client(host);
        if !client.active() || !client.prefs.outline || client.preview.len() <= 1 {
            return &[];
        }
        let limit = client.prefs.outline_limit.min(RIM_BUDGET);
        &client.preview[..limit.min(client.preview.len())]
    }

    pub fn visuals(&mut self, host: &dyn Host, rims: &mut Vec<BlockRim>) {
        rims.extend(
            self.outlined(host)
                .iter()
                .map(|&pos| BlockRim::new(pos, 0xC0FF_FFFF, 2.0)),
        );
    }

    /// The targeted block's vanilla outline: replaced by the preview's rim
    /// when the preview outlines that block, so the whole vein looks the
    /// same. Drawn after the rims and on top of them, it would hide the
    /// targeted block's rim under its black lines.
    pub fn replaces_highlight(&mut self, host: &dyn Host, pos: Pos) -> bool {
        self.outlined(host).contains(&pos)
    }

    /// The overlay, top left: the shape, the count, and the shapes around
    /// the current one (Ultimine's menu).
    pub fn hud(&mut self, host: &dyn Host, draw: &mut dyn DrawContext) {
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
        let order: Vec<ShapeId> = ShapeId::ALL
            .into_iter()
            .filter(|s| rules.allows(*s))
            .collect();
        let at = order.iter().position(|s| *s == current).unwrap_or(0) as i32;
        let mut lines = vec![(title, 0xFFFF_FFFF)];
        if order.len() > 1 {
            let span = (order.len() as i32 - 1).min(4) / 2;
            for d in -span..=span {
                let shape = order[(at + d).rem_euclid(order.len() as i32) as usize];
                let (mark, argb) = if d == 0 {
                    ("> ", 0xFFFF_FF55)
                } else {
                    ("  ", 0xFFAA_AAAA)
                };
                lines.push((format!("{mark}{}", shape.label()), argb));
            }
        }
        if client.rules.is_none() {
            lines.push(("(server without Ultimine)".into(), 0xFFFF_5555));
        }
        let width = lines
            .iter()
            .map(|(t, _)| draw.text_width(t))
            .fold(0.0, f32::max)
            + 6.0;
        let height = lines.len() as f32 * 10.0 + 4.0;
        let (x, y) = (2.0, 2.0);
        draw.fill([x, y, width, height], 0x7800_0000);
        for (i, (text, argb)) in lines.iter().enumerate() {
            draw.text(text, [x + 3.0, y + 3.0 + i as f32 * 10.0], *argb, true);
        }
    }
}
