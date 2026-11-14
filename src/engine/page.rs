//! The settings page (Options > Mods > Ultimine): one typed setting per
//! option, a toggle or a choice. The activation key is listed in Controls by
//! Horizon itself. The page edits this game's own settings: the player's
//! preferences, and the rules of the worlds it hosts (single player and
//! LAN). A dedicated server keeps its rules in its own data folder.
//!
//! Labels are translation keys, `ultimine.setting.<key>` and
//! `ultimine.option.<value>`, in `assets/ultimine/lang/en_us.json`.
use super::config::{Activation, ClientPrefs, Rules, default_merge_groups};
use super::shape::ShapeId;

const MAX_BLOCKS: [usize; 8] = [16, 32, 64, 128, 256, 512, 1024, 2048];
const EXHAUSTION: [f32; 6] = [0.0, 5.0, 10.0, 20.0, 40.0, 80.0];
const COOLDOWN: [u32; 5] = [0, 10, 20, 40, 100];

/// What a setting is, with its default.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Toggle(bool),
    /// The options' translation keys and the default's index.
    Choice(Vec<String>, usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Setting {
    pub key: String,
    /// Its label's translation key.
    pub label: String,
    pub kind: Kind,
}

/// A setting's value as the game keeps it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Value {
    Toggle(bool),
    Choice(usize),
}

/// The shape the player scrolled to: a choice, kept when they scroll.
pub const SHAPE: &str = "shape";

fn toggle(key: &str, default: bool) -> Setting {
    Setting {
        key: key.into(),
        label: format!("ultimine.setting.{key}"),
        kind: Kind::Toggle(default),
    }
}

fn choice(key: &str, options: Vec<String>, default: usize) -> Setting {
    Setting {
        key: key.into(),
        label: format!("ultimine.setting.{key}"),
        kind: Kind::Choice(options, default),
    }
}

fn options<T: ToString>(values: impl IntoIterator<Item = T>) -> Vec<String> {
    values
        .into_iter()
        .map(|v| format!("ultimine.option.{}", v.to_string().replace('.', "_")))
        .collect()
}

fn index_of<T: PartialEq>(values: &[T], value: &T) -> usize {
    values.iter().position(|v| v == value).unwrap_or(0)
}

fn activation_key(a: Activation) -> &'static str {
    match a {
        Activation::HoldKey => "hold_key",
        Activation::Sneak => "sneak",
        Activation::KeyAndSneak => "key_and_sneak",
    }
}

/// Every setting, in the page's order, with Ultimine's defaults.
pub fn settings() -> Vec<Setting> {
    let (prefs, rules) = (ClientPrefs::default(), Rules::default());
    let mut out = vec![
        choice(
            "activation",
            options(Activation::ALL.map(activation_key)),
            index_of(&Activation::ALL, &prefs.activation),
        ),
        toggle("outline", prefs.outline),
        toggle("hud", prefs.hud),
        choice(
            SHAPE,
            options(ShapeId::ALL.map(ShapeId::key)),
            prefs.shape.index(),
        ),
        choice(
            "max_blocks",
            options(MAX_BLOCKS),
            index_of(&MAX_BLOCKS, &rules.max_blocks),
        ),
        choice(
            "exhaustion",
            options(EXHAUSTION.map(|e| format!("hunger_{e}"))),
            index_of(&EXHAUSTION, &rules.exhaustion_per_block),
        ),
        choice(
            "cooldown",
            options(COOLDOWN.map(|c| format!("ticks_{c}"))),
            index_of(&COOLDOWN, &rules.cooldown_ticks),
        ),
        toggle("shaped_any_block", rules.shaped_any_block),
        toggle("merge_groups", !rules.merge_groups.is_empty()),
        toggle("drops_to_inventory", rules.drops_to_inventory),
        toggle("right_click", rules.right_click_axe),
    ];
    out.extend(ShapeId::ALL.map(|s| toggle(&format!("shape_{}", s.key()), rules.allows(s))));
    out
}

/// The player's preferences and the rules as the settings hold them; `get`
/// answers a key's value (its default when the player never changed it).
pub fn read(get: &dyn Fn(&str) -> Option<Value>) -> (ClientPrefs, Rules) {
    let on = |key: &str, default: bool| match get(key) {
        Some(Value::Toggle(b)) => b,
        _ => default,
    };
    let pick = |key: &str, len: usize| match get(key) {
        Some(Value::Choice(i)) if i < len => Some(i),
        _ => None,
    };
    let mut prefs = ClientPrefs::default();
    let mut rules = Rules::default();
    if let Some(i) = pick("activation", Activation::ALL.len()) {
        prefs.activation = Activation::ALL[i];
    }
    prefs.outline = on("outline", prefs.outline);
    prefs.hud = on("hud", prefs.hud);
    if let Some(i) = pick(SHAPE, ShapeId::ALL.len()) {
        prefs.shape = ShapeId::ALL[i];
    }
    if let Some(i) = pick("max_blocks", MAX_BLOCKS.len()) {
        rules.max_blocks = MAX_BLOCKS[i];
    }
    if let Some(i) = pick("exhaustion", EXHAUSTION.len()) {
        rules.exhaustion_per_block = EXHAUSTION[i];
    }
    if let Some(i) = pick("cooldown", COOLDOWN.len()) {
        rules.cooldown_ticks = COOLDOWN[i];
    }
    rules.shaped_any_block = on("shaped_any_block", rules.shaped_any_block);
    rules.merge_groups = if on("merge_groups", true) {
        default_merge_groups()
    } else {
        Vec::new()
    };
    rules.drops_to_inventory = on("drops_to_inventory", rules.drops_to_inventory);
    let right_click = on("right_click", rules.right_click_axe);
    rules.right_click_axe = right_click;
    rules.right_click_shovel = right_click;
    rules.right_click_hoe = right_click;
    rules.right_click_harvest = right_click;
    let shapes: Vec<ShapeId> = ShapeId::ALL
        .into_iter()
        .filter(|s| on(&format!("shape_{}", s.key()), true))
        .collect();
    // The last shape can't be turned off: with none, all are on.
    rules.shapes = if shapes.is_empty() {
        ShapeId::ALL.to_vec()
    } else {
        shapes
    };
    (prefs, rules)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_read_back_as_ultimines() {
        let (prefs, rules) = read(&|_| None);
        assert_eq!(prefs, ClientPrefs::default());
        assert_eq!(rules, Rules::default());
        let keys: Vec<String> = settings().into_iter().map(|s| s.key).collect();
        assert_eq!(keys.len(), 17);
        let mut unique = keys.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), keys.len());
    }

    #[test]
    fn every_setting_reads_into_valid_rules() {
        for setting in settings() {
            let values: Vec<Value> = match &setting.kind {
                Kind::Toggle(_) => vec![Value::Toggle(false), Value::Toggle(true)],
                Kind::Choice(options, _) => (0..options.len() + 1).map(Value::Choice).collect(),
            };
            for value in values {
                let key = setting.key.clone();
                let (_, rules) = read(&|k| (k == key).then_some(value));
                rules.validate().unwrap();
            }
        }
        let (prefs, rules) = read(&|k| match k {
            "max_blocks" => Some(Value::Choice(3)),
            SHAPE => Some(Value::Choice(3)),
            "drops_to_inventory" => Some(Value::Toggle(true)),
            _ => None,
        });
        assert_eq!(rules.max_blocks, 128);
        assert_eq!(prefs.shape, ShapeId::LargeTunnel);
        assert!(rules.drops_to_inventory);
    }

    #[test]
    fn the_last_shape_cannot_be_turned_off() {
        let (_, rules) = read(&|k| k.starts_with("shape_").then_some(Value::Toggle(false)));
        assert_eq!(rules.shapes, ShapeId::ALL.to_vec());
        let (_, rules) = read(&|k| {
            (k.starts_with("shape_") && k != "shape_small_tunnel").then_some(Value::Toggle(false))
        });
        assert_eq!(rules.shapes, vec![ShapeId::SmallTunnel]);
    }

    #[test]
    fn every_label_is_translated() {
        let lang = include_str!("../../assets/ultimine/lang/en_us.json");
        for setting in settings() {
            assert!(
                lang.contains(&format!("\"{}\"", setting.label)),
                "{}",
                setting.label
            );
            if let Kind::Choice(options, _) = &setting.kind {
                for option in options {
                    assert!(lang.contains(&format!("\"{option}\"")), "{option}");
                }
            }
        }
    }
}
