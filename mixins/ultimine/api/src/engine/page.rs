//! The settings page (Options > Mods > Ultimine): one button per option,
//! each click steps it to its next value. The activation key is listed above
//! these, rebindable, by Horizon itself. The page edits this game's own
//! files: the player's preferences, and the rules of the worlds it hosts
//! (single player and LAN). A dedicated server keeps its rules in its own
//! folder.
use super::config::{Activation, ClientPrefs, Rules, default_merge_groups};
use super::shape::ShapeId;

const MAX_BLOCKS: [usize; 8] = [16, 32, 64, 128, 256, 512, 1024, 2048];
const EXHAUSTION: [f32; 6] = [0.0, 5.0, 10.0, 20.0, 40.0, 80.0];
const COOLDOWN: [u32; 5] = [0, 10, 20, 40, 100];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Button {
    Activation,
    Outline,
    Hud,
    MaxBlocks,
    Exhaustion,
    Cooldown,
    ShapedMatch,
    MergeGroups,
    Drops,
    RightClick,
    Shape(ShapeId),
}

fn buttons() -> Vec<Button> {
    let mut out = vec![
        Button::Activation,
        Button::Outline,
        Button::Hud,
        Button::MaxBlocks,
        Button::Exhaustion,
        Button::Cooldown,
        Button::ShapedMatch,
        Button::MergeGroups,
        Button::Drops,
        Button::RightClick,
    ];
    out.extend(ShapeId::ALL.map(Button::Shape));
    out
}

fn on(b: bool) -> &'static str {
    if b { "On" } else { "Off" }
}

fn next<T: PartialEq + Copy>(values: &[T], current: T) -> T {
    let at = values.iter().position(|v| *v == current);
    values[at.map_or(0, |i| (i + 1) % values.len())]
}

fn right_click(rules: &Rules) -> bool {
    rules.right_click_axe
        || rules.right_click_shovel
        || rules.right_click_hoe
        || rules.right_click_harvest
}

pub fn labels(prefs: &ClientPrefs, rules: &Rules) -> Vec<String> {
    buttons()
        .into_iter()
        .map(|b| match b {
            Button::Activation => format!("Activation: {}", prefs.activation.label()),
            Button::Outline => format!("Outline preview: {}", on(prefs.outline)),
            Button::Hud => format!("Shape overlay: {}", on(prefs.hud)),
            Button::MaxBlocks => format!("Max blocks: {}", rules.max_blocks),
            Button::Exhaustion => format!("Hunger per block: {}", rules.exhaustion_per_block),
            Button::Cooldown => format!("Cooldown: {} ticks", rules.cooldown_ticks),
            Button::ShapedMatch => format!(
                "Shaped mining takes: {}",
                if rules.shaped_any_block {
                    "Any block"
                } else {
                    "Same block"
                }
            ),
            Button::MergeGroups => format!(
                "Merge ores, stone and logs: {}",
                on(!rules.merge_groups.is_empty())
            ),
            Button::Drops => format!(
                "Drops: {}",
                if rules.drops_to_inventory {
                    "To inventory"
                } else {
                    "In the world"
                }
            ),
            Button::RightClick => format!(
                "Right-click till/strip/path/harvest: {}",
                on(right_click(rules))
            ),
            Button::Shape(s) => format!("Shape {}: {}", s.label(), on(rules.allows(s))),
        })
        .collect()
}

/// What a click changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Changed {
    Nothing,
    Prefs,
    Rules,
}

pub fn click(prefs: &mut ClientPrefs, rules: &mut Rules, index: usize) -> Changed {
    let Some(button) = buttons().get(index).copied() else {
        return Changed::Nothing;
    };
    match button {
        Button::Activation => prefs.activation = next(&Activation::ALL, prefs.activation),
        Button::Outline => prefs.outline = !prefs.outline,
        Button::Hud => prefs.hud = !prefs.hud,
        Button::MaxBlocks => rules.max_blocks = next(&MAX_BLOCKS, rules.max_blocks),
        Button::Exhaustion => {
            rules.exhaustion_per_block = next(&EXHAUSTION, rules.exhaustion_per_block)
        }
        Button::Cooldown => rules.cooldown_ticks = next(&COOLDOWN, rules.cooldown_ticks),
        Button::ShapedMatch => rules.shaped_any_block = !rules.shaped_any_block,
        Button::MergeGroups => {
            rules.merge_groups = if rules.merge_groups.is_empty() {
                default_merge_groups()
            } else {
                Vec::new()
            }
        }
        Button::Drops => rules.drops_to_inventory = !rules.drops_to_inventory,
        Button::RightClick => {
            let to = !right_click(rules);
            rules.right_click_axe = to;
            rules.right_click_shovel = to;
            rules.right_click_hoe = to;
            rules.right_click_harvest = to;
        }
        Button::Shape(s) => {
            if rules.allows(s) {
                if rules.shapes.len() == 1 {
                    return Changed::Nothing;
                }
                rules.shapes.retain(|x| *x != s);
            } else {
                rules.shapes.push(s);
                rules.shapes.sort();
            }
        }
    }
    match button {
        Button::Activation | Button::Outline | Button::Hud => Changed::Prefs,
        _ => Changed::Rules,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_button_steps_and_stays_valid() {
        let mut prefs = ClientPrefs::default();
        let mut rules = Rules::default();
        let count = labels(&prefs, &rules).len();
        assert_eq!(count, 16);
        for i in 0..count {
            for _ in 0..10 {
                let before = labels(&prefs, &rules)[i].clone();
                let changed = click(&mut prefs, &mut rules, i);
                rules.validate().unwrap();
                if changed != Changed::Nothing {
                    assert_ne!(before, labels(&prefs, &rules)[i], "button {i}");
                }
            }
        }
        assert_eq!(click(&mut prefs, &mut rules, 99), Changed::Nothing);
    }

    #[test]
    fn the_last_shape_cannot_be_turned_off() {
        let mut prefs = ClientPrefs::default();
        let mut rules = Rules {
            shapes: vec![ShapeId::SmallTunnel],
            ..Rules::default()
        };
        let small_tunnel = 10 + 1;
        assert_eq!(
            click(&mut prefs, &mut rules, small_tunnel),
            Changed::Nothing
        );
        assert_eq!(click(&mut prefs, &mut rules, 10), Changed::Rules);
        assert_eq!(rules.shapes, vec![ShapeId::Shapeless, ShapeId::SmallTunnel]);
        assert_eq!(labels(&prefs, &rules)[3], "Max blocks: 64");
    }
}
