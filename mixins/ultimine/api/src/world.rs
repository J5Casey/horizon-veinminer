//! The engine's traits over Horizon's `Host`.
use crate::engine::geom::Pos;
use crate::engine::matcher::Tags;
use crate::engine::rightclick::{Action, Farmer};
use crate::engine::select::World;
use crate::engine::server::Breaker;
use crate::player_break::BREAK;
use horizon_client::mod_api::Host;

/// Block reads (`Host::block`: air where nothing is loaded, so unloaded
/// blocks never join a selection).
pub struct HostWorld<'a>(pub &'a dyn Host);

impl World for HostWorld<'_> {
    fn block(&self, pos: Pos) -> Option<String> {
        Some(self.0.block(pos))
    }
}

/// Tag membership from the server's registries.
pub struct HostTags<'a>(pub &'a dyn Host);

impl Tags for HostTags<'_> {
    fn in_tag(&self, block: &str, tag: &str) -> Option<bool> {
        self.0.block_name_in_tag(block, tag).ok()
    }
}

/// The acting player breaking blocks.
pub struct HostBreaker<'a> {
    pub host: &'a mut dyn Host,
    pub uuid: u128,
    pub into_inventory: bool,
}

impl Breaker for HostBreaker<'_> {
    fn block(&self, pos: Pos) -> Option<String> {
        Some(self.host.block(pos))
    }
    fn held(&self) -> Option<String> {
        self.host.held().map(|(item, _, _)| item)
    }
    fn food(&self) -> Option<i32> {
        self.host.player_food(self.uuid).map(|(food, _, _)| food)
    }
    fn break_block(&mut self, pos: Pos) -> bool {
        BREAK.break_block(self.host, self.uuid, pos, self.into_inventory)
    }
    fn exhaust(&mut self, amount: f32) {
        exhaust(self.host, self.uuid, amount);
    }
}

/// Adds exhaustion as `FoodData.addExhaustion` does (capped at 40).
pub fn exhaust(host: &mut dyn Host, uuid: u128, amount: f32) {
    if let Some((food, saturation, exhaustion)) = host.player_food(uuid) {
        let total = (exhaustion + amount).min(40.0);
        host.set_player_status(uuid, None, Some((food, saturation, total)), None);
    }
}

/// `Block.UPDATE_ALL`.
const UPDATE_ALL: i32 = 3;

/// Right-click actions in the acting player's level.
pub struct HostFarmer<'a> {
    pub host: &'a mut dyn Host,
    pub uuid: u128,
    pub dimension: String,
    pub into_inventory: bool,
    pub creative: bool,
}

impl HostFarmer<'_> {
    fn set(&mut self, pos: Pos, block: &str, keep: &[(&str, Option<String>)]) -> bool {
        let Some(state) = self.host.default_state(block) else {
            return false;
        };
        if !self.host.set_block(&self.dimension, pos, state, UPDATE_ALL) {
            return false;
        }
        for (key, value) in keep {
            if let Some(value) = value {
                self.host.set_block_property(pos, key, value);
            }
        }
        true
    }
}

impl Farmer for HostFarmer<'_> {
    fn block(&self, pos: Pos) -> Option<String> {
        Some(self.host.block(pos))
    }
    fn air_above(&self, pos: Pos) -> bool {
        pos[1]
            .checked_add(1)
            .is_some_and(|y| crate::engine::blocks::is_air(&self.host.block([pos[0], y, pos[2]])))
    }
    fn age(&self, pos: Pos) -> Option<u8> {
        self.host.block_property(pos, "age")?.parse().ok()
    }
    fn apply(&mut self, pos: Pos, action: &Action) -> bool {
        match action {
            Action::Strip(to) => {
                let axis = self.host.block_property(pos, "axis");
                self.set(pos, to, &[("axis", axis)])
            }
            Action::Flatten => self.set(pos, "minecraft:dirt_path", &[]),
            Action::Till(to) => self.set(pos, to, &[]),
            Action::Harvest(crop) => {
                let keep: Vec<(&str, Option<String>)> = crop
                    .keep
                    .iter()
                    .map(|key| (*key, self.host.block_property(pos, key)))
                    .collect();
                if !BREAK.break_block(self.host, self.uuid, pos, self.into_inventory) {
                    return false;
                }
                // Replanting costs one seed, as in Ultimine; with none to
                // hand the crop is simply harvested.
                if self.creative || self.host.take_items(crop.seed, 1) == 1 {
                    self.set(pos, crop.block, &keep);
                }
                true
            }
        }
    }
}
