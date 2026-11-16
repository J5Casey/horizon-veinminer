//! Settings. `Rules` are the server's (what may be mined and what it costs);
//! the server sends them to each client so its preview matches. `ClientPrefs`
//! are the player's own (how Ultimine is activated and shown).
use super::json::{self, Json};
use super::matcher::Group;
use super::shape::ShapeId;

pub const MAX_BLOCKS_LIMIT: usize = 4096;

#[derive(Clone, Debug, PartialEq)]
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

/// The rules file's keys, in `Rules`' order: the names its fields had in
/// Ultimine 0.4's `rules` JSON, which this file keeps.
pub const RULE_KEYS: [&str; 16] = [
    "max_blocks",
    "exhaustion_per_block",
    "cooldown_ticks",
    "shapes",
    "shaped_any_block",
    "merge_groups",
    "merge_groups_shaped",
    "excluded_blocks",
    "only_blocks",
    "require_tool",
    "drops_to_inventory",
    "stop_on_break_fail",
    "right_click_axe",
    "right_click_shovel",
    "right_click_hoe",
    "right_click_harvest",
];

/// The keys of the custom block lists, what the default rules file holds.
pub const LIST_KEYS: [&str; 4] = [
    "merge_groups",
    "merge_groups_shaped",
    "excluded_blocks",
    "only_blocks",
];

fn whole(key: &str, value: &Json, max: u64) -> Result<u64, String> {
    match value {
        Json::Number(n) if n.fract() == 0.0 && (0.0..=max as f64).contains(n) => Ok(*n as u64),
        other => Err(format!(
            "{key}: expected a whole number 0..={max}, got {}",
            other.kind()
        )),
    }
}

fn boolean(key: &str, value: &Json) -> Result<bool, String> {
    match value {
        Json::Bool(b) => Ok(*b),
        other => Err(format!(
            "{key}: expected true or false, got {}",
            other.kind()
        )),
    }
}

fn strings(key: &str, value: &Json) -> Result<Vec<String>, String> {
    let Json::Array(items) = value else {
        return Err(format!(
            "{key}: expected an array of strings, got {}",
            value.kind()
        ));
    };
    items
        .iter()
        .map(|item| match item {
            Json::String(s) => Ok(s.clone()),
            other => Err(format!(
                "{key}: expected an array of strings, got {} in it",
                other.kind()
            )),
        })
        .collect()
}

fn groups(key: &str, value: &Json) -> Result<Vec<Group>, String> {
    let Json::Array(items) = value else {
        return Err(format!(
            "{key}: expected an array of arrays of strings, got {}",
            value.kind()
        ));
    };
    items.iter().map(|group| strings(key, group)).collect()
}

fn write_strings(ids: &[String]) -> String {
    let ids: Vec<String> = ids.iter().map(|id| json::quote(id)).collect();
    format!("[{}]", ids.join(", "))
}

impl Rules {
    /// Sets the fields the rules file `text` names, keeping the others: the
    /// JSON object Ultimine 0.4 kept as its `rules` setting, every key
    /// optional. Fails closed: an unknown key, a wrong type or rules that
    /// don't validate refuse the whole file and change nothing.
    pub fn apply_json(&mut self, text: &str) -> Result<(), String> {
        let Json::Object(members) = Json::parse(text)? else {
            return Err("expected a JSON object".into());
        };
        let mut rules = self.clone();
        for (key, value) in &members {
            let key = key.as_str();
            match key {
                "max_blocks" => {
                    rules.max_blocks = whole(key, value, MAX_BLOCKS_LIMIT as u64)? as usize
                }
                "exhaustion_per_block" => match value {
                    Json::Number(n) => rules.exhaustion_per_block = *n as f32,
                    other => return Err(format!("{key}: expected a number, got {}", other.kind())),
                },
                "cooldown_ticks" => {
                    rules.cooldown_ticks = whole(key, value, u32::MAX.into())? as u32
                }
                "shapes" => {
                    rules.shapes = strings(key, value)?
                        .iter()
                        .map(|s| {
                            ShapeId::from_key(s).ok_or_else(|| format!("shapes: no shape {s:?}"))
                        })
                        .collect::<Result<_, _>>()?;
                }
                "shaped_any_block" => rules.shaped_any_block = boolean(key, value)?,
                "merge_groups" => rules.merge_groups = groups(key, value)?,
                "merge_groups_shaped" => rules.merge_groups_shaped = groups(key, value)?,
                "excluded_blocks" => rules.excluded_blocks = strings(key, value)?,
                "only_blocks" => rules.only_blocks = strings(key, value)?,
                "require_tool" => rules.require_tool = boolean(key, value)?,
                "drops_to_inventory" => rules.drops_to_inventory = boolean(key, value)?,
                "stop_on_break_fail" => rules.stop_on_break_fail = boolean(key, value)?,
                "right_click_axe" => rules.right_click_axe = boolean(key, value)?,
                "right_click_shovel" => rules.right_click_shovel = boolean(key, value)?,
                "right_click_hoe" => rules.right_click_hoe = boolean(key, value)?,
                "right_click_harvest" => rules.right_click_harvest = boolean(key, value)?,
                _ => return Err(format!("unknown key {key:?}")),
            }
        }
        rules.validate()?;
        *self = rules;
        Ok(())
    }

    /// The fields named by `keys` (of [`RULE_KEYS`]) as a rules file, in the
    /// format `apply_json` reads.
    pub fn to_json(&self, keys: &[&str]) -> String {
        let mut members = Vec::new();
        for key in RULE_KEYS.into_iter().filter(|k| keys.contains(k)) {
            let value = match key {
                "max_blocks" => self.max_blocks.to_string(),
                "exhaustion_per_block" => format!("{:?}", self.exhaustion_per_block),
                "cooldown_ticks" => self.cooldown_ticks.to_string(),
                "shapes" => {
                    let shapes: Vec<String> =
                        self.shapes.iter().map(|s| s.key().to_owned()).collect();
                    write_strings(&shapes)
                }
                "shaped_any_block" => self.shaped_any_block.to_string(),
                "merge_groups" | "merge_groups_shaped" => {
                    let groups = if key == "merge_groups" {
                        &self.merge_groups
                    } else {
                        &self.merge_groups_shaped
                    };
                    if groups.is_empty() {
                        "[]".to_owned()
                    } else {
                        let lines: Vec<String> = groups
                            .iter()
                            .map(|g| format!("    {}", write_strings(g)))
                            .collect();
                        format!("[\n{}\n  ]", lines.join(",\n"))
                    }
                }
                "excluded_blocks" => write_strings(&self.excluded_blocks),
                "only_blocks" => write_strings(&self.only_blocks),
                "require_tool" => self.require_tool.to_string(),
                "drops_to_inventory" => self.drops_to_inventory.to_string(),
                "stop_on_break_fail" => self.stop_on_break_fail.to_string(),
                "right_click_axe" => self.right_click_axe.to_string(),
                "right_click_shovel" => self.right_click_shovel.to_string(),
                "right_click_hoe" => self.right_click_hoe.to_string(),
                _ => self.right_click_harvest.to_string(),
            };
            members.push(format!("  {}: {value}", json::quote(key)));
        }
        format!("{{\n{}\n}}\n", members.join(",\n"))
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
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

#[derive(Clone, Debug, PartialEq)]
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
    }

    #[test]
    fn rules_files_read_as_ultimine_0_4_wrote_them() {
        let rules = Rules::default();
        let mut back = Rules {
            max_blocks: 1,
            shapes: vec![ShapeId::SmallTunnel],
            merge_groups: Vec::new(),
            right_click_harvest: false,
            ..Rules::default()
        };
        back.apply_json(&rules.to_json(&RULE_KEYS)).unwrap();
        assert_eq!(back, rules);
        let mut same = rules.clone();
        same.apply_json("{}").unwrap();
        assert_eq!(same, rules);
        // A 0.4 file: the custom lists, and a field the page also sets.
        let mut custom = Rules::default();
        custom
            .apply_json(
                r##"{
                  "max_blocks": 128,
                  "shapes": ["shapeless", "large_tunnel"],
                  "merge_groups": [["#c:ores/iron", "minecraft:raw_iron_block"]],
                  "excluded_blocks": ["minecraft:spawner"],
                  "only_blocks": []
                }"##,
            )
            .unwrap();
        assert_eq!(custom.max_blocks, 128);
        assert_eq!(custom.shapes, [ShapeId::Shapeless, ShapeId::LargeTunnel]);
        assert_eq!(
            custom.merge_groups,
            [vec![
                "#c:ores/iron".to_owned(),
                "minecraft:raw_iron_block".into()
            ]]
        );
        assert_eq!(custom.excluded_blocks, ["minecraft:spawner"]);
        assert_eq!(custom.exhaustion_per_block, rules.exhaustion_per_block);
        let lists = rules.to_json(&LIST_KEYS);
        assert!(lists.contains("\"merge_groups\": [\n    [\"minecraft:stone\""));
        assert!(!lists.contains("max_blocks"));
    }

    #[test]
    fn bad_rules_files_change_nothing() {
        for text in [
            r#"{"max_blocks":0}"#,
            r#"{"max_blocks":100000}"#,
            r#"{"max_blocks":1.5}"#,
            r#"{"exhaustion_per_block":-1}"#,
            r#"{"shapes":[]}"#,
            r#"{"shapes":["round"]}"#,
            r#"{"excluded_blocks":["stone"]}"#,
            r#"{"excluded_blocks":"minecraft:stone"}"#,
            r##"{"merge_groups":[["#Bad:tag"]]}"##,
            r#"{"merge_groups":["minecraft:stone"]}"#,
            r#"{"require_tool":1}"#,
            r#"{"unknown":1}"#,
            "[]",
            "not json",
        ] {
            let mut rules = Rules::default();
            assert!(rules.apply_json(text).is_err(), "{text}");
            assert_eq!(rules, Rules::default(), "{text}");
        }
    }

    #[test]
    fn bad_rules_fail_closed() {
        let bad = [
            Rules {
                max_blocks: 0,
                ..Rules::default()
            },
            Rules {
                max_blocks: 100_000,
                ..Rules::default()
            },
            Rules {
                exhaustion_per_block: -1.0,
                ..Rules::default()
            },
            Rules {
                shapes: Vec::new(),
                ..Rules::default()
            },
            Rules {
                excluded_blocks: vec!["stone".into()],
                ..Rules::default()
            },
            Rules {
                merge_groups: vec![vec!["#Bad:tag".into()]],
                ..Rules::default()
            },
        ];
        for rules in bad {
            assert!(rules.validate().is_err(), "{rules:?}");
        }
        Rules {
            merge_groups: vec![vec![
                "#c:ores/iron".into(),
                "minecraft:raw_iron_block".into(),
            ]],
            ..Rules::default()
        }
        .validate()
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
    }
}
