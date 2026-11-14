//! Ultimine's shapes. Each one lists the positions to break, in the order
//! they break, from the broken block, the face it was hit on and a check
//! that says whether a position may join. The geometry follows Ultimine's
//! observable behaviour (its README and in-game results); the code is ours.
use super::geom::{self, Axis, Face, Pos};
use std::collections::{BTreeSet, VecDeque};

/// The shapes, in Ultimine's registration (and cycling) order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ShapeId {
    #[default]
    Shapeless,
    SmallTunnel,
    SmallSquare,
    LargeTunnel,
    MiningTunnel,
    EscapeTunnel,
}

impl ShapeId {
    pub const ALL: [ShapeId; 6] = [
        ShapeId::Shapeless,
        ShapeId::SmallTunnel,
        ShapeId::SmallSquare,
        ShapeId::LargeTunnel,
        ShapeId::MiningTunnel,
        ShapeId::EscapeTunnel,
    ];

    pub fn key(self) -> &'static str {
        match self {
            ShapeId::Shapeless => "shapeless",
            ShapeId::SmallTunnel => "small_tunnel",
            ShapeId::SmallSquare => "small_square",
            ShapeId::LargeTunnel => "large_tunnel",
            ShapeId::MiningTunnel => "mining_tunnel",
            ShapeId::EscapeTunnel => "escape_tunnel",
        }
    }

    /// The shape named `key` (as [`ShapeId::key`] writes it).
    pub fn from_key(key: &str) -> Option<ShapeId> {
        ShapeId::ALL.into_iter().find(|s| s.key() == key)
    }

    /// The shape's place in [`ShapeId::ALL`].
    pub fn index(self) -> usize {
        ShapeId::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    pub fn label(self) -> &'static str {
        match self {
            ShapeId::Shapeless => "Shapeless",
            ShapeId::SmallTunnel => "Small Tunnel",
            ShapeId::SmallSquare => "Small Square (3x3)",
            ShapeId::LargeTunnel => "Large Tunnel (3x3)",
            ShapeId::MiningTunnel => "Mining Tunnel",
            ShapeId::EscapeTunnel => "Escape Tunnel",
        }
    }

    pub fn shape(self) -> &'static dyn Shape {
        match self {
            ShapeId::Shapeless => &Shapeless,
            ShapeId::SmallTunnel => &Line,
            ShapeId::SmallSquare => &Layers { max_depth: 1 },
            ShapeId::LargeTunnel => &Layers {
                max_depth: usize::MAX,
            },
            ShapeId::MiningTunnel => &DiagonalTunnel { rise: -1 },
            ShapeId::EscapeTunnel => &DiagonalTunnel { rise: 1 },
        }
    }
}

/// What a shape works from.
pub struct ShapeContext<'a> {
    pub origin: Pos,
    /// The face of `origin` the player hit.
    pub face: Face,
    /// The horizontal direction the player faces.
    pub facing: Face,
    pub max_blocks: usize,
    /// Plants spread through Ultimine's plant neighbourhood (a 5x5 layer
    /// plus the blocks straight above and below) rather than the 3x3x3 cube.
    pub plant: bool,
    pub check: &'a dyn Fn(Pos) -> bool,
}

impl ShapeContext<'_> {
    fn check(&self, pos: Pos) -> bool {
        (self.check)(pos)
    }
}

pub trait Shape: Sync {
    /// The positions to break, `origin` first when it qualifies. Never more
    /// than `max_blocks`; empty when `origin` itself doesn't qualify.
    fn blocks(&self, ctx: &ShapeContext<'_>) -> Vec<Pos>;
    /// Shapeless mining follows the vein, so its matcher may merge groups.
    fn indeterminate(&self) -> bool {
        false
    }
}

/// Every matching block connected to the origin through its 26 neighbours
/// (or the plant neighbourhood), nearest first.
pub struct Shapeless;

fn cube() -> Vec<[i32; 3]> {
    let mut out = Vec::with_capacity(26);
    for x in -1..=1 {
        for y in -1..=1 {
            for z in -1..=1 {
                if [x, y, z] != [0, 0, 0] {
                    out.push([x, y, z]);
                }
            }
        }
    }
    out
}

fn plant_neighbourhood() -> Vec<[i32; 3]> {
    let mut out = Vec::with_capacity(26);
    for x in -2..=2 {
        for z in -2..=2 {
            if [x, z] != [0, 0] {
                out.push([x, 0, z]);
            }
        }
    }
    out.push([0, 1, 0]);
    out.push([0, -1, 0]);
    out.sort_by_key(|d| geom::dist_sq(*d, [0, 0, 0]));
    out
}

impl Shape for Shapeless {
    fn blocks(&self, ctx: &ShapeContext<'_>) -> Vec<Pos> {
        let steps = if ctx.plant {
            plant_neighbourhood()
        } else {
            cube()
        };
        let mut found: Vec<Pos> = Vec::new();
        let mut seen = BTreeSet::from([ctx.origin]);
        let mut open = VecDeque::from([ctx.origin]);
        while let Some(pos) = open.pop_front() {
            if found.len() >= ctx.max_blocks {
                break;
            }
            if !ctx.check(pos) {
                continue;
            }
            found.push(pos);
            for step in &steps {
                if let Some(next) = geom::add(pos, *step)
                    && seen.insert(next)
                {
                    open.push_back(next);
                }
            }
        }
        // Nearest first, ties in discovery order, as Ultimine sorts its set.
        found.sort_by_key(|p| geom::dist_sq(*p, ctx.origin));
        found.truncate(ctx.max_blocks);
        found
    }

    fn indeterminate(&self) -> bool {
        true
    }
}

/// A one-block line straight into the face hit, until a block doesn't match.
pub struct Line;

impl Shape for Line {
    fn blocks(&self, ctx: &ShapeContext<'_>) -> Vec<Pos> {
        let into = ctx.face.opposite().normal();
        let mut out = Vec::new();
        for i in 0..ctx.max_blocks {
            let Some(pos) = i32::try_from(i)
                .ok()
                .and_then(|i| geom::offset(ctx.origin, into, i))
            else {
                break;
            };
            if !ctx.check(pos) {
                break;
            }
            out.push(pos);
        }
        out
    }
}

/// 3x3 layers across the face hit, going into it: one layer is the small
/// square, unlimited is the large tunnel. A layer where nothing matches ends
/// the tunnel.
pub struct Layers {
    pub max_depth: usize,
}

impl Shape for Layers {
    fn blocks(&self, ctx: &ShapeContext<'_>) -> Vec<Pos> {
        if ctx.max_blocks == 0 || !ctx.check(ctx.origin) {
            return Vec::new();
        }
        let into = ctx.face.opposite().normal();
        let mut out = vec![ctx.origin];
        let mut base = ctx.origin;
        let mut depth = 0;
        'tunnel: while depth < self.max_depth && out.len() < ctx.max_blocks {
            let before = out.len();
            for a in -1..=1 {
                for b in -1..=1 {
                    if depth == 0 && a == 0 && b == 0 {
                        continue;
                    }
                    let delta = match ctx.face.axis() {
                        Axis::X => [0, a, b],
                        Axis::Y => [a, 0, b],
                        Axis::Z => [a, b, 0],
                    };
                    let Some(pos) = geom::add(base, delta) else {
                        continue;
                    };
                    if ctx.check(pos) {
                        out.push(pos);
                        if out.len() >= ctx.max_blocks {
                            break 'tunnel;
                        }
                    }
                }
            }
            if out.len() == before {
                break;
            }
            let Some(next) = geom::add(base, into) else {
                break;
            };
            base = next;
            depth += 1;
        }
        out
    }
}

/// A one-block diagonal, one step forward and `rise` up per block: down for
/// the mining tunnel, up for the escape tunnel. Forward is into the face hit,
/// or the way the player looks when they hit a top or bottom.
pub struct DiagonalTunnel {
    pub rise: i32,
}

impl Shape for DiagonalTunnel {
    fn blocks(&self, ctx: &ShapeContext<'_>) -> Vec<Pos> {
        let forward = if ctx.face.is_vertical() {
            ctx.facing
        } else {
            ctx.face.opposite()
        };
        let [dx, _, dz] = forward.normal();
        let step = [dx, self.rise, dz];
        let mut out = Vec::new();
        for i in 0..ctx.max_blocks {
            let Some(pos) = i32::try_from(i)
                .ok()
                .and_then(|i| geom::offset(ctx.origin, step, i))
            else {
                break;
            };
            if !ctx.check(pos) {
                break;
            }
            out.push(pos);
        }
        out
    }
}
