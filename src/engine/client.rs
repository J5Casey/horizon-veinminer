//! The client half's state: activation from the key (or sneaking), the
//! shape the player scrolls to, what to tell the server, and the preview.
use super::config::{ClientPrefs, Rules};
use super::geom::{Face, Pos};
use super::matcher::NoTags;
use super::protocol::{ClientState, VERSION};
use super::select::{Request, World, select};
use super::shape::ShapeId;

/// Ticks between repeated requests for the rules while the server hasn't
/// answered (a server without Ultimine never does).
const RULES_RETRY: u32 = 100;

#[derive(Default)]
pub struct Client {
    pub prefs: ClientPrefs,
    pub key_down: bool,
    pub sneaking: bool,
    /// The server's rules; `None` until it answers.
    pub rules: Option<Rules>,
    sent: Option<(bool, ShapeId)>,
    since_ask: u32,
    /// What the preview was computed for, and its blocks in breaking order.
    pub preview_for: Option<Request>,
    pub preview: Vec<Pos>,
}

impl Client {
    pub fn new(prefs: ClientPrefs) -> Self {
        Client {
            prefs,
            ..Client::default()
        }
    }

    pub fn active(&self) -> bool {
        self.prefs.activation.active(self.key_down, self.sneaking)
    }

    /// The rules in force: the server's, else Ultimine's defaults.
    pub fn rules(&self) -> Rules {
        self.rules.clone().unwrap_or_default()
    }

    pub fn shape(&self) -> ShapeId {
        self.rules().effective_shape(self.prefs.shape)
    }

    /// Wheel steps while playing. Returns true when Ultimine takes them
    /// (it is active), so the hotbar doesn't scroll. Up goes back a shape.
    pub fn scroll(&mut self, steps: f32) -> bool {
        if !self.active() || !steps.is_finite() || steps == 0.0 {
            return false;
        }
        let by = if steps > 0.0 { -1 } else { 1 };
        self.prefs.shape = self.rules().cycle(self.shape(), by);
        true
    }

    /// What to send the server this tick, if anything: changes of state at
    /// once, and a request for the rules now and then until they come.
    pub fn outgoing(&mut self) -> Option<ClientState> {
        let now = (self.active(), self.shape());
        let want_rules =
            self.rules.is_none() && (self.sent.is_none() || self.since_ask >= RULES_RETRY);
        self.since_ask = self.since_ask.saturating_add(1);
        if self.sent == Some(now) && !want_rules {
            return None;
        }
        if want_rules {
            self.since_ask = 0;
        }
        self.sent = Some(now);
        Some(ClientState {
            v: VERSION,
            active: now.0,
            shape: now.1,
            want_rules,
        })
    }

    /// The connection ended: the next server is asked afresh.
    pub fn disconnected(&mut self) {
        self.rules = None;
        self.sent = None;
        self.since_ask = 0;
        self.clear_preview();
    }

    pub fn clear_preview(&mut self) {
        self.preview_for = None;
        self.preview.clear();
    }

    /// Recomputes the preview for the block the crosshair is on. The client
    /// has no tags, so tag entries of merge groups don't match here; the
    /// server's selection is the one that counts.
    pub fn update_preview(&mut self, world: &dyn World, target: Option<(Pos, Face)>, facing: Face) {
        let Some((origin, face)) = target.filter(|_| self.active()) else {
            self.clear_preview();
            return;
        };
        let request = Request {
            origin,
            face,
            facing,
            shape: self.shape(),
        };
        let selection = select(world, &self.rules(), &NoTags, &request);
        self.preview_for = Some(request);
        self.preview = selection.positions;
    }
}
