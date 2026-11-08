//! Settings. `Rules` are the server's (what may be mined and what it costs);
//! the server sends them to each client so its preview matches. `ClientPrefs`
//! are the player's own (how Ultimine is activated and shown).
use super::matcher::Group;
use super::shape::ShapeId;
use serde::{Deserialize, Serialize};

pub const MAX_BLOCKS_LIMIT: usize = 4096;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Rules {
    /// Most blocks one use breaks, the broken block included.
    pub max_blocks: usize,
    /// Ultimine's hunger cost, per block: 20 is 0.1 exhaustion (x0.005).
    pub exhaustion_per_block: f32,
    /// Ticks between two uses.
    pub cooldown_ticks: u32,
    /// The shapes players may pick, in Ultimine's cycling order.
    pub shapes: Vec<ShapeId>,
    /// Shaped mining takes any block (Ultimine's default `merge_tags_shaped
    /// = ["*"]`); off, it uses `merge_groups_shaped`, else the same block.
    pub shaped_any_block: bool,
    /// Blocks that shapeless mining treats as one (ids or `#tags`).
    pub merge_groups: Vec<Group>,
    /// The same for shaped mining when `shaped_any_block` is off.
    pub merge_groups_shaped: Vec<Group>,
    /// Never mined (Ultimine's `excluded_blocks` tag).
    pub excluded_blocks: Vec<String>,
    /// When not empty, only these are mined (Ultimine's `block_whitelist`).
    pub only_blocks: Vec<String>,
    /// Require a tool-like item in the main hand.
    pub require_tool: bool,
    /// Give the drops straight to the player instead of dropping them.
    pub drops_to_inventory: bool,
    /// Stop at the first block that fails to break instead of skipping it.
    pub stop_on_break_fail: bool,
    pub right_click_axe: bool,
    pub right_click_shovel: bool,
    pub right_click_hoe: bool,
    pub right_click_harvest: bool,
}

fn group(names: &[&str]) -> Group {
    names.iter().map(|n| format!("minecraft:{n}")).collect()
}

/// Ultimine merges `minecraft:base_stone_overworld` and each `c:ores/<type>`
/// tag; these are those tags' vanilla members written out, so a client
/// preview (which has no tags) agrees with the server.
pub fn default_merge_groups() -> Vec<Group> {
    let mut groups = vec![group(&[
        "stone",
        "granite",
        "diorite",
        "andesite",
        "tuff",
        "deepslate",
    ])];
    for ore in [
        "coal", "copper", "iron", "gold", "redstone", "lapis", "diamond", "emerald",
    ] {
        groups.push(vec![
            format!("minecraft:{ore}_ore"),
            format!("minecraft:deepslate_{ore}_ore"),
        ]);
    }
    // Tree felling: a species' logs and bark blocks come down together.
    for wood in [
        "oak", "spruce", "birch", "jungle", "acacia", "dark_oak", "mangrove", "cherry", "pale_oak",
    ] {
        groups.push(vec![
            format!("minecraft:{wood}_log"),
            format!("minecraft:{wood}_wood"),
        ]);
    }
    for stem in ["crimson", "warped"] {
        groups.push(vec![
            format!("minecraft:{stem}_stem"),
            format!("minecraft:{stem}_hyphae"),
        ]);
    }
    groups
}

impl Default for Rules {
    fn default() -> Self {
        Rules {
            max_blocks: 64,
            exhaustion_per_block: 20.0,
            cooldown_ticks: 0,
            shapes: ShapeId::ALL.to_vec(),
            shaped_any_block: true,
            merge_groups: default_merge_groups(),
            merge_groups_shaped: Vec::new(),
            excluded_blocks: Vec::new(),
            only_blocks: Vec::new(),
            require_tool: false,
            drops_to_inventory: false,
            stop_on_break_fail: false,
            right_click_axe: true,
            right_click_shovel: true,
            right_click_hoe: true,
            right_click_harvest: true,
        }
    }
}

fn id_valid(id: &str) -> bool {
    let id = id.strip_prefix('#').unwrap_or(id);
    let Some((namespace, path)) = id.split_once(':') else {
        return false;
    };
    !namespace.is_empty()
        && !path.is_empty()
        && namespace
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b))
        && path
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"/_.-".contains(&b))
}

impl Rules {
    pub fn from_json(text: &str) -> Result<Self, String> {
        let rules: Rules = serde_json::from_str(text).map_err(|e| e.to_string())?;
        rules.validate()?;
        Ok(rules)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Fails closed: an unusable file disables nothing silently, it is refused.
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=MAX_BLOCKS_LIMIT).contains(&self.max_blocks) {
            return Err(format!("max_blocks must be 1..={MAX_BLOCKS_LIMIT}"));
        }
        if !self.exhaustion_per_block.is_finite()
            || !(0.0..=10_000.0).contains(&self.exhaustion_per_block)
        {
            return Err("exhaustion_per_block must be 0..=10000".into());
        }
        if self.cooldown_ticks > 72_000 {
            return Err("cooldown_ticks must be at most 72000".into());
        }
        if self.shapes.is_empty() {
            return Err("at least one shape must be enabled".into());
        }
        let groups = self.merge_groups.iter().chain(&self.merge_groups_shaped);
        if self.merge_groups.len() + self.merge_groups_shaped.len() > 512 {
            return Err("too many merge groups".into());
        }
        let ids = groups
            .flatten()
            .chain(&self.excluded_blocks)
            .chain(&self.only_blocks);
        let mut count = 0usize;
        for id in ids {
            count += 1;
            if count > 8192 {
                return Err("too many block ids".into());
            }
            if !id_valid(id) {
                return Err(format!(
                    "expected a namespaced block id or #tag, got {id:?}"
                ));
            }
        }
        Ok(())
    }

    /// Whether the player may pick `shape`.
    pub fn allows(&self, shape: ShapeId) -> bool {
        self.shapes.contains(&shape)
    }

    /// `shape` if allowed, else the first allowed shape.
    pub fn effective_shape(&self, shape: ShapeId) -> ShapeId {
        if self.allows(shape) {
            shape
        } else {
            self.shapes.first().copied().unwrap_or(ShapeId::Shapeless)
        }
    }

    /// The allowed shape `steps` places after `shape` (backwards when
    /// negative), wrapping around.
    pub fn cycle(&self, shape: ShapeId, steps: i32) -> ShapeId {
        let order: Vec<ShapeId> = ShapeId::ALL
            .into_iter()
            .filter(|s| self.allows(*s))
            .collect();
        if order.is_empty() {
            return shape;
        }
        let at = order.iter().position(|s| *s == shape).unwrap_or(0) as i64;
        let len = order.len() as i64;
        order[(at + i64::from(steps)).rem_euclid(len) as usize]
    }

    /// Exhaustion added per extra block, as vanilla's food units.
    pub fn exhaustion(&self) -> f32 {
        self.exhaustion_per_block * 0.005
    }
}

/// How the player turns Ultimine on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Activation {
    /// While the Ultimine key is held (Ultimine's behaviour).
    #[default]
    HoldKey,
    /// While sneaking, no key needed.
    Sneak,
    /// Only while the key is held and the player sneaks.
    KeyAndSneak,
}

impl Activation {
    pub const ALL: [Activation; 3] = [
        Activation::HoldKey,
        Activation::Sneak,
        Activation::KeyAndSneak,
    ];

    pub fn active(self, key: bool, sneaking: bool) -> bool {
        match self {
            Activation::HoldKey => key,
            Activation::Sneak => sneaking,
            Activation::KeyAndSneak => key && sneaking,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Activation::HoldKey => "Hold key",
            Activation::Sneak => "Sneak",
            Activation::KeyAndSneak => "Key + sneak",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ClientPrefs {
    pub activation: Activation,
    pub shape: ShapeId,
    /// Outline the blocks that will break.
    pub outline: bool,
    /// Most blocks outlined (Ultimine's `render_outline`).
    pub outline_limit: usize,
    /// Show the shape box over the HUD while active.
    pub hud: bool,
}

impl Default for ClientPrefs {
    fn default() -> Self {
        ClientPrefs {
            activation: Activation::HoldKey,
            shape: ShapeId::Shapeless,
            outline: true,
            outline_limit: 256,
            hud: true,
        }
    }
}

impl ClientPrefs {
    /// Never fails: an unreadable file means the defaults.
    pub fn from_json(text: &str) -> Self {
        serde_json::from_str(text).unwrap_or_default()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_ultimines() {
        let rules = Rules::default();
        assert_eq!(rules.max_blocks, 64);
        assert_eq!(rules.exhaustion_per_block, 20.0);
        assert!((rules.exhaustion() - 0.1).abs() < 1e-6);
        assert!(rules.shaped_any_block);
        assert_eq!(rules.shapes, ShapeId::ALL.to_vec());
        rules.validate().unwrap();
        assert_eq!(Rules::from_json(&rules.to_json()).unwrap(), rules);
        assert_eq!(Rules::from_json("{}").unwrap(), rules);
    }

    #[test]
    fn bad_rules_fail_closed() {
        for text in [
            r#"{"max_blocks":0}"#,
            r#"{"max_blocks":100000}"#,
            r#"{"exhaustion_per_block":-1}"#,
            r#"{"shapes":[]}"#,
            r#"{"shapes":["round"]}"#,
            r#"{"excluded_blocks":["stone"]}"#,
            r##"{"merge_groups":[["#Bad:tag"]]}"##,
            r#"{"unknown":1}"#,
            "not json",
        ] {
            assert!(Rules::from_json(text).is_err(), "{text}");
        }
        Rules::from_json(r##"{"merge_groups":[["#c:ores/iron","minecraft:raw_iron_block"]]}"##)
            .unwrap();
    }

    #[test]
    fn cycling_skips_disabled_shapes_and_wraps() {
        let rules = Rules {
            shapes: vec![
                ShapeId::Shapeless,
                ShapeId::LargeTunnel,
                ShapeId::EscapeTunnel,
            ],
            ..Rules::default()
        };
        assert_eq!(rules.cycle(ShapeId::Shapeless, 1), ShapeId::LargeTunnel);
        assert_eq!(rules.cycle(ShapeId::Shapeless, -1), ShapeId::EscapeTunnel);
        assert_eq!(rules.cycle(ShapeId::EscapeTunnel, 2), ShapeId::LargeTunnel);
        assert_eq!(
            rules.effective_shape(ShapeId::SmallTunnel),
            ShapeId::Shapeless
        );
        assert_eq!(rules.cycle(ShapeId::SmallTunnel, 1), ShapeId::LargeTunnel);
    }

    #[test]
    fn activation_modes() {
        assert!(Activation::HoldKey.active(true, false));
        assert!(!Activation::HoldKey.active(false, true));
        assert!(Activation::Sneak.active(false, true));
        assert!(!Activation::KeyAndSneak.active(true, false));
        assert!(Activation::KeyAndSneak.active(true, true));
        assert_eq!(ClientPrefs::from_json("garbage"), ClientPrefs::default());
    }
}
