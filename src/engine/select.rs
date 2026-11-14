//! Selection: which blocks one use of Ultimine takes. Pure; the client runs
//! it for the preview, the server again (authoritatively) before breaking.
use super::blocks;
use super::config::Rules;
use super::geom::{Face, Pos};
use super::matcher::{MatchKind, Matcher, Tags};
use super::shape::{ShapeContext, ShapeId};

/// Reads blocks by name. `None` is a block that isn't loaded: never part of
/// a selection, and reading never loads a chunk.
pub trait World {
    fn block(&self, pos: Pos) -> Option<String>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request {
    pub origin: Pos,
    pub face: Face,
    /// The player's horizontal facing (`Face::from_yaw`).
    pub facing: Face,
    pub shape: ShapeId,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    /// In breaking order, the origin first.
    pub positions: Vec<Pos>,
    /// The origin's block name.
    pub origin_block: String,
    pub kind: Option<MatchKind>,
}

impl Selection {
    pub fn len(&self) -> usize {
        self.positions.len()
    }
    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }
}

pub fn select(world: &dyn World, rules: &Rules, tags: &dyn Tags, req: &Request) -> Selection {
    let shape_id = rules.effective_shape(req.shape);
    let shape = shape_id.shape();
    let Some(origin_block) = world.block(req.origin) else {
        return Selection::default();
    };
    if blocks::is_air(&origin_block) {
        return Selection::default();
    }
    let matcher = Matcher::choose(rules, tags, &origin_block, shape.indeterminate());
    let check = |pos: Pos| world.block(pos).is_some_and(|name| matcher.matches(&name));
    let ctx = ShapeContext {
        origin: req.origin,
        face: req.face,
        facing: req.facing,
        max_blocks: rules.max_blocks,
        plant: matcher.kind == MatchKind::Plant,
        check: &check,
    };
    let positions = shape.blocks(&ctx);
    Selection {
        kind: (!positions.is_empty()).then_some(matcher.kind),
        positions,
        origin_block,
    }
}
