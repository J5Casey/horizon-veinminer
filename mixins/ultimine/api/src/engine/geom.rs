//! Block positions and faces, with checked arithmetic so a selection near the
//! integer limits never wraps around to the far side of the world.
use serde::{Deserialize, Serialize};

pub type Pos = [i32; 3];

/// One of a block's six faces, named as vanilla's `Direction`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Face {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

/// The axis a face lies across.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Face {
    pub const ALL: [Face; 6] = [
        Face::Down,
        Face::Up,
        Face::North,
        Face::South,
        Face::West,
        Face::East,
    ];

    pub fn normal(self) -> [i32; 3] {
        match self {
            Face::Down => [0, -1, 0],
            Face::Up => [0, 1, 0],
            Face::North => [0, 0, -1],
            Face::South => [0, 0, 1],
            Face::West => [-1, 0, 0],
            Face::East => [1, 0, 0],
        }
    }

    pub fn from_normal(normal: [i32; 3]) -> Option<Face> {
        Face::ALL.into_iter().find(|f| f.normal() == normal)
    }

    pub fn opposite(self) -> Face {
        match self {
            Face::Down => Face::Up,
            Face::Up => Face::Down,
            Face::North => Face::South,
            Face::South => Face::North,
            Face::West => Face::East,
            Face::East => Face::West,
        }
    }

    pub fn axis(self) -> Axis {
        match self {
            Face::Down | Face::Up => Axis::Y,
            Face::North | Face::South => Axis::Z,
            Face::West | Face::East => Axis::X,
        }
    }

    pub fn is_vertical(self) -> bool {
        self.axis() == Axis::Y
    }

    /// The horizontal direction an entity with this yaw faces, as vanilla's
    /// `Direction.fromYRot`: 0 is south, 90 west, 180 north, 270 east.
    pub fn from_yaw(yaw: f32) -> Face {
        let quarter = if yaw.is_finite() {
            (f64::from(yaw) / 90.0 + 0.5).floor() as i64
        } else {
            0
        };
        match quarter.rem_euclid(4) {
            0 => Face::South,
            1 => Face::West,
            2 => Face::North,
            _ => Face::East,
        }
    }

    /// The face of block `block` that the point `point` (where a view ray met
    /// it) lies on: the side whose plane the point is closest to.
    pub fn from_hit(block: Pos, point: [f64; 3]) -> Face {
        let local = [
            point[0] - f64::from(block[0]),
            point[1] - f64::from(block[1]),
            point[2] - f64::from(block[2]),
        ];
        let candidates = [
            (local[1], Face::Down),
            (1.0 - local[1], Face::Up),
            (local[2], Face::North),
            (1.0 - local[2], Face::South),
            (local[0], Face::West),
            (1.0 - local[0], Face::East),
        ];
        candidates
            .into_iter()
            .filter(|(d, _)| d.is_finite())
            .min_by(|a, b| a.0.abs().total_cmp(&b.0.abs()))
            .map_or(Face::Up, |(_, f)| f)
    }
}

/// `pos + step * k`, or `None` when any coordinate would overflow.
pub fn offset(pos: Pos, step: [i32; 3], k: i32) -> Option<Pos> {
    Some([
        pos[0].checked_add(step[0].checked_mul(k)?)?,
        pos[1].checked_add(step[1].checked_mul(k)?)?,
        pos[2].checked_add(step[2].checked_mul(k)?)?,
    ])
}

/// `pos + delta`, or `None` on overflow.
pub fn add(pos: Pos, delta: [i32; 3]) -> Option<Pos> {
    offset(pos, delta, 1)
}

/// Squared distance between two positions, wide enough for any pair.
pub fn dist_sq(a: Pos, b: Pos) -> i128 {
    (0..3)
        .map(|i| {
            let d = i128::from(a[i]) - i128::from(b[i]);
            d * d
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaw_matches_vanilla_direction() {
        assert_eq!(Face::from_yaw(0.0), Face::South);
        assert_eq!(Face::from_yaw(44.0), Face::South);
        assert_eq!(Face::from_yaw(46.0), Face::West);
        assert_eq!(Face::from_yaw(180.0), Face::North);
        assert_eq!(Face::from_yaw(-90.0), Face::East);
        assert_eq!(Face::from_yaw(270.0), Face::East);
        assert_eq!(Face::from_yaw(f32::NAN), Face::South);
    }

    #[test]
    fn hit_points_name_their_face() {
        assert_eq!(Face::from_hit([3, 4, 5], [3.5, 5.0, 5.5]), Face::Up);
        assert_eq!(Face::from_hit([3, 4, 5], [3.5, 4.0, 5.5]), Face::Down);
        assert_eq!(Face::from_hit([3, 4, 5], [3.0, 4.5, 5.5]), Face::West);
        assert_eq!(Face::from_hit([-3, 4, -5], [-2.0, 4.5, -4.5]), Face::East);
        assert_eq!(Face::from_hit([0, 0, 0], [0.5, 0.5, 1.0]), Face::South);
    }

    #[test]
    fn offsets_never_wrap() {
        assert_eq!(offset([i32::MAX, 0, 0], [1, 0, 0], 1), None);
        assert_eq!(offset([0, 0, 0], [0, -1, 0], 3), Some([0, -3, 0]));
        assert_eq!(
            dist_sq([i32::MIN, 0, 0], [i32::MAX, 0, 0]),
            i128::from(u32::MAX).pow(2)
        );
    }

    #[test]
    fn faces_round_trip() {
        for f in Face::ALL {
            assert_eq!(Face::from_normal(f.normal()), Some(f));
            assert_eq!(f.opposite().opposite(), f);
        }
    }
}
