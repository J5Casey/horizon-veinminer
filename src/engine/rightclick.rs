//! Right-click variants: with Ultimine active, a right click strips every
//! selected log (axe), flattens paths (shovel), tills (hoe) or harvests and
//! replants ripe crops (any item), as Ultimine's right-click handlers do.
use super::blocks::{self, Crop};
use super::config::Rules;
use super::geom::Pos;
use super::select::Selection;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolKind {
    Axe,
    Shovel,
    Hoe,
    Other,
}

pub fn tool_kind(item: Option<&str>) -> ToolKind {
    let path = item.map_or("", |i| i.rsplit(':').next().unwrap_or(i));
    if path.ends_with("_axe") {
        ToolKind::Axe
    } else if path.ends_with("_shovel") {
        ToolKind::Shovel
    } else if path.ends_with("_hoe") {
        ToolKind::Hoe
    } else {
        ToolKind::Other
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Becomes another block, keeping `axis`.
    Strip(String),
    /// Becomes a dirt path.
    Flatten,
    /// Becomes farmland (or dirt, from coarse dirt).
    Till(&'static str),
    /// Drops its harvest and starts again at age 0.
    Harvest(&'static Crop),
}

/// What a right click with `tool` does to `block`. `air_above` and `age`
/// are read only when an action needs them.
pub fn action(
    rules: &Rules,
    tool: ToolKind,
    block: &str,
    air_above: impl FnOnce() -> bool,
    age: impl FnOnce() -> Option<u8>,
) -> Option<Action> {
    if let Some(crop) = blocks::crop(block) {
        return (rules.right_click_harvest && age() == Some(crop.ripe_age))
            .then_some(Action::Harvest(crop));
    }
    let (wanted, to) = match tool {
        ToolKind::Axe if rules.right_click_axe => {
            return blocks::stripped(block).map(Action::Strip);
        }
        ToolKind::Shovel if rules.right_click_shovel && blocks::flattens(block) => {
            (true, Action::Flatten)
        }
        ToolKind::Hoe if rules.right_click_hoe => (true, Action::Till(blocks::tilled(block)?)),
        _ => (false, Action::Flatten),
    };
    (wanted && air_above()).then_some(to)
}

/// The world calls right-click actions make.
pub trait Farmer {
    fn block(&self, pos: Pos) -> Option<String>;
    fn air_above(&self, pos: Pos) -> bool;
    fn age(&self, pos: Pos) -> Option<u8>;
    fn apply(&mut self, pos: Pos, action: &Action) -> bool;
}

/// Applies the right-click action to each selected block it suits. The
/// number of blocks changed.
pub fn apply(
    farmer: &mut dyn Farmer,
    selection: &Selection,
    rules: &Rules,
    tool: ToolKind,
) -> usize {
    let mut done = 0;
    for &pos in &selection.positions {
        let Some(block) = farmer.block(pos) else {
            continue;
        };
        let found = action(
            rules,
            tool,
            &block,
            || farmer.air_above(pos),
            || farmer.age(pos),
        );
        if let Some(found) = found
            && farmer.apply(pos, &found)
        {
            done += 1;
        }
    }
    done
}
