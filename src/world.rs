//! The engine's traits over the mod API's server host: block reads in the
//! acting player's level, tags, breaking as the player (`PlayerRef::destroy_block`)
//! with the vein's drops kept by `block_drops`, and the right-click actions.
use crate::engine::geom::Pos;
use crate::engine::matcher::Tags;
use crate::engine::rightclick::{Action, Farmer};
use crate::engine::select::World;
use crate::engine::server::Breaker;
use crate::gather::{self, Kept};
use mod_api::{
    BlockUpdateFlags, EquipmentSlot, Host, HostTransaction, ItemPlace, ItemResource, ItemStack,
    LevelRef, LivingRef, PlayerRef, ServerHost,
};

/// The id of the block at `pos` (air where nothing is loaded, so unloaded
/// blocks never join a selection).
pub fn block_id(host: &dyn Host, level: LevelRef, pos: Pos) -> String {
    level
        .block(host, pos)
        .map_or_else(|| "minecraft:air".to_owned(), |b| b.id(host))
}

/// The item `player` holds in the main hand.
pub fn held(host: &dyn Host, player: PlayerRef) -> Option<String> {
    let stack = LivingRef::new(player.entity())
        .equipment(host, EquipmentSlot::MainHand)
        .filter(|s| s.count() > 0)?;
    Some(stack.item().id(host))
}

/// Block reads in one level.
pub struct HostWorld<'a> {
    pub host: &'a dyn Host,
    pub level: LevelRef,
}

impl World for HostWorld<'_> {
    fn block(&self, pos: Pos) -> Option<String> {
        Some(block_id(self.host, self.level, pos))
    }
}

/// Tag membership from the server's registries.
pub struct HostTags<'a>(pub &'a dyn Host);

impl Tags for HostTags<'_> {
    fn in_tag(&self, block: &str, tag: &str) -> Option<bool> {
        let key = self.0.block_key(block)?;
        self.0.block_in_tag(key, tag)
    }
}

/// The acting player breaking blocks. `block_drops` keeps each break's
/// drops and experience (see `gather`); they are set down at the origin
/// when the use ends.
pub struct HostBreaker<'a> {
    pub host: &'a mut dyn ServerHost,
    pub player: PlayerRef,
    pub level: LevelRef,
    pub into_inventory: bool,
}

impl Breaker for HostBreaker<'_> {
    fn block(&self, pos: Pos) -> Option<String> {
        Some(block_id(self.host, self.level, pos))
    }
    fn held(&self) -> Option<String> {
        held(self.host, self.player)
    }
    fn food(&self) -> Option<i32> {
        self.player.food(self.host).map(|f| f.level)
    }
    fn break_block(&mut self, pos: Pos) -> bool {
        gather::expect(pos);
        // The drops are kept by `block_drops` whatever `into_inventory` says, so
        // they can be merged first; they reach the inventory in `drop_gathered`.
        self.player.destroy_block(self.host, pos, false)
    }
    fn drop_gathered(&mut self, origin: Pos) {
        let kept = gather::take();
        set_down(
            self.host,
            self.player,
            self.level,
            origin,
            kept,
            self.into_inventory,
        );
    }
    fn exhaust(&mut self, amount: f32) {
        self.player.add_exhaustion(self.host, amount);
    }
}

/// Folds each stack into the earlier ones it stacks with, up to the item's
/// stack size, in the order they came: as few stacks as fit.
pub fn merge(host: &dyn Host, stacks: Vec<ItemStack>) -> Vec<ItemStack> {
    let mut merged: Vec<ItemStack> = Vec::new();
    for stack in stacks {
        let max = host.max_stack_size(stack.item()).unwrap_or(64);
        let mut rest = stack.count();
        for into in merged.iter_mut().filter(|m| m.is_stackable_with(&stack)) {
            let room = max.saturating_sub(into.count()).min(rest);
            if room > 0 && into.grow(room).is_ok() {
                rest -= room;
            }
        }
        if rest > 0 {
            let mut left = stack.clone();
            if left.set_count(rest).is_ok() {
                merged.push(left);
            }
        }
    }
    merged
}

/// The main inventory's slots (`inventory_tick`'s 0..36, the hotbar first).
const INVENTORY_SLOTS: i32 = 36;

/// `player`'s main inventory, slot by slot (`player_inventory` lists only
/// the non-empty stacks, without their slots).
fn inventory(host: &mut dyn ServerHost, player: u128) -> Vec<Option<ItemStack>> {
    (0..INVENTORY_SLOTS)
        .map(|slot| {
            ItemPlace::PlayerSlot { player, slot }
                .stack(host)
                .filter(|s| s.count() > 0)
        })
        .collect()
}

/// Puts `stack` into `player`'s main inventory as `Inventory.add` does: onto
/// stacks of the same item first, then into empty slots. What went in.
fn insert(host: &mut dyn ServerHost, player: PlayerRef, stack: &ItemStack) -> u32 {
    let uuid = player.uuid();
    let slots = inventory(host, uuid);
    let max = host.max_stack_size(stack.item()).unwrap_or(64);
    let resource = ItemResource::of(stack);
    let same = slots.iter().enumerate().filter_map(|(slot, held)| {
        let held = held.as_ref().filter(|s| s.is_stackable_with(stack))?;
        Some((slot, max.saturating_sub(held.count())))
    });
    let empty = slots
        .iter()
        .enumerate()
        .filter(|(_, held)| held.is_none())
        .map(|(slot, _)| (slot, max));
    let places: Vec<(usize, u32)> = same.chain(empty).collect();
    let mut left = stack.count();
    for (slot, room) in places {
        let amount = room.min(left);
        if amount == 0 {
            continue;
        }
        let place = ItemPlace::PlayerSlot {
            player: uuid,
            slot: slot as i32,
        };
        let went = host
            .transaction(|host, tx| Ok::<_, ()>(place.insert(host, tx, &resource, amount)))
            .unwrap_or(0);
        left -= went.min(left);
    }
    stack.count() - left
}

/// Sets what a use kept down at `origin`, merged into as few stacks as fit;
/// with `into_inventory` the player gets what fits first and the rest lands
/// at `origin`. The experience lands at the origin's centre either way.
pub fn set_down(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    level: LevelRef,
    origin: Pos,
    kept: Kept,
    into_inventory: bool,
) {
    for mut stack in merge(host, kept.stacks) {
        if into_inventory {
            let went = insert(host, player, &stack);
            eprintln!(
                "ultimine-debug: insert {} went {went} inv {:?}",
                stack.count(),
                player.inventory(host).map(|i| i.len())
            );
            if went >= stack.count() || stack.set_count(stack.count() - went).is_err() {
                continue;
            }
        }
        if level.pop_resource(host, origin, stack).is_err() {
            eprintln!("ultimine: a kept drop could not be set down at {origin:?}");
        }
    }
    if kept.experience > 0 {
        let [x, y, z] = origin.map(|v| f64::from(v) + 0.5);
        level.award_experience(host, x, y, z, kept.experience);
    }
}

/// Right-click actions in the acting player's level.
pub struct HostFarmer<'a> {
    pub host: &'a mut dyn ServerHost,
    pub player: PlayerRef,
    pub level: LevelRef,
    pub into_inventory: bool,
    pub creative: bool,
}

impl HostFarmer<'_> {
    fn set(&mut self, pos: Pos, block: &str, keep: &[(&str, Option<String>)]) -> bool {
        let Some(state) = self
            .host
            .block_key(block)
            .and_then(|key| self.host.block_state_default(key))
        else {
            return false;
        };
        let flags = BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS;
        if !matches!(self.level.set_block(self.host, pos, state, flags), Ok(true)) {
            return false;
        }
        for (key, value) in keep {
            if let Some(value) = value {
                self.host.set_block_property(pos, key, value);
            }
        }
        true
    }

    /// Takes one `item` from the player's main inventory; false when they
    /// have none.
    fn take_one(&mut self, item: &str) -> bool {
        let uuid = self.player.uuid();
        let slots = inventory(self.host, uuid);
        let Some((slot, stack)) = slots.iter().enumerate().find_map(|(slot, s)| {
            Some((slot, s.as_ref()?)).filter(|(_, s)| s.item().id(self.host) == item)
        }) else {
            return false;
        };
        let place = ItemPlace::PlayerSlot {
            player: uuid,
            slot: slot as i32,
        };
        let resource = ItemResource::of(stack);
        self.host
            .transaction(|host, tx| Ok::<_, ()>(place.extract(host, tx, &resource, 1)))
            .unwrap_or(0)
            == 1
    }
}

impl Farmer for HostFarmer<'_> {
    fn block(&self, pos: Pos) -> Option<String> {
        Some(block_id(self.host, self.level, pos))
    }
    fn air_above(&self, pos: Pos) -> bool {
        pos[1].checked_add(1).is_some_and(|y| {
            crate::engine::blocks::is_air(&block_id(self.host, self.level, [pos[0], y, pos[2]]))
        })
    }
    fn age(&self, pos: Pos) -> Option<u8> {
        self.level
            .block_property(self.host, pos, "age")?
            .parse()
            .ok()
    }
    fn apply(&mut self, pos: Pos, action: &Action) -> bool {
        match action {
            Action::Strip(to) => {
                let axis = self.level.block_property(self.host, pos, "axis");
                self.set(pos, to, &[("axis", axis)])
            }
            Action::Flatten => self.set(pos, "minecraft:dirt_path", &[]),
            Action::Till(to) => self.set(pos, to, &[]),
            Action::Harvest(crop) => {
                let keep: Vec<(&str, Option<String>)> = crop
                    .keep
                    .iter()
                    .map(|key| (*key, self.level.block_property(self.host, pos, key)))
                    .collect();
                if !self
                    .player
                    .destroy_block(self.host, pos, self.into_inventory)
                {
                    return false;
                }
                // Replanting costs one seed, as in Ultimine; with none to
                // hand the crop is simply harvested.
                if self.creative || self.take_one(crop.seed) {
                    self.set(pos, crop.block, &keep);
                }
                true
            }
        }
    }
}
