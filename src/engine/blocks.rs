//! What the engine knows about blocks by name alone. Horizon's stable helpers
//! name blocks (`minecraft:stone`) but don't expose hardness or block
//! classes, so the few facts the selection needs are kept here as small,
//! explicit lists rather than guessed.

/// Air of any kind: never part of a selection.
pub fn is_air(name: &str) -> bool {
    matches!(
        name,
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
    )
}

/// Fluids and fluid-like blocks, which "any block" matching skips as Ultimine
/// skips `LiquidBlock`s.
pub fn is_fluid(name: &str) -> bool {
    matches!(
        name,
        "minecraft:water" | "minecraft:lava" | "minecraft:bubble_column"
    )
}

/// Blocks a survival player can't break (vanilla hardness -1) or that only
/// exist as technical blocks. Ultimine skips them through the hardness check.
pub fn is_unbreakable(name: &str) -> bool {
    matches!(
        name,
        "minecraft:bedrock"
            | "minecraft:barrier"
            | "minecraft:light"
            | "minecraft:command_block"
            | "minecraft:chain_command_block"
            | "minecraft:repeating_command_block"
            | "minecraft:structure_block"
            | "minecraft:structure_void"
            | "minecraft:jigsaw"
            | "minecraft:test_block"
            | "minecraft:test_instance_block"
            | "minecraft:end_portal"
            | "minecraft:end_portal_frame"
            | "minecraft:end_gateway"
            | "minecraft:nether_portal"
            | "minecraft:moving_piston"
            | "minecraft:reinforced_deepslate"
    )
}

/// Much harder than ordinary terrain. Ultimine refuses, in a shaped mine, any
/// block harder than the one broken (`destroySpeed <= baseSpeed`); without a
/// hardness helper these are the blocks that check exists for, so a 3x3
/// through stone never takes the obsidian behind it. Mined only when the
/// broken block is the same block.
pub fn is_hard(name: &str) -> bool {
    matches!(
        name,
        "minecraft:obsidian"
            | "minecraft:crying_obsidian"
            | "minecraft:respawn_anchor"
            | "minecraft:ancient_debris"
            | "minecraft:netherite_block"
            | "minecraft:enchanting_table"
            | "minecraft:ender_chest"
            | "minecraft:spawner"
            | "minecraft:trial_spawner"
            | "minecraft:vault"
            | "minecraft:heavy_core"
    )
}

/// A crop that right-click harvesting resets: its ripe `age`, the item that
/// replants it (one is taken from the drops) and properties to keep.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Crop {
    pub block: &'static str,
    pub ripe_age: u8,
    pub seed: &'static str,
    pub keep: &'static [&'static str],
}

pub const CROPS: &[Crop] = &[
    Crop {
        block: "minecraft:wheat",
        ripe_age: 7,
        seed: "minecraft:wheat_seeds",
        keep: &[],
    },
    Crop {
        block: "minecraft:carrots",
        ripe_age: 7,
        seed: "minecraft:carrot",
        keep: &[],
    },
    Crop {
        block: "minecraft:potatoes",
        ripe_age: 7,
        seed: "minecraft:potato",
        keep: &[],
    },
    Crop {
        block: "minecraft:beetroots",
        ripe_age: 3,
        seed: "minecraft:beetroot_seeds",
        keep: &[],
    },
    Crop {
        block: "minecraft:cocoa",
        ripe_age: 2,
        seed: "minecraft:cocoa_beans",
        keep: &["facing"],
    },
];

pub fn crop(name: &str) -> Option<&'static Crop> {
    CROPS.iter().find(|c| c.block == name)
}

/// Ultimine's "bush type": plants of one type select together with the
/// plant-shaped neighbourhood. `None` for a block that isn't plant-like.
pub fn plant_kind(name: &str) -> Option<u8> {
    let path = name.strip_prefix("minecraft:")?;
    if crop(name).is_some_and(|c| c.block != "minecraft:cocoa")
        || matches!(
            path,
            "torchflower_crop" | "pitcher_crop" | "melon_stem" | "pumpkin_stem"
        )
    {
        return Some(1);
    }
    if path.ends_with("_sapling") || path == "mangrove_propagule" {
        return Some(2);
    }
    if path == "cocoa" {
        return Some(3);
    }
    if path == "sweet_berry_bush" {
        return Some(4);
    }
    let vegetation = matches!(
        path,
        "short_grass"
            | "tall_grass"
            | "fern"
            | "large_fern"
            | "dead_bush"
            | "short_dry_grass"
            | "tall_dry_grass"
            | "bush"
            | "firefly_bush"
            | "dandelion"
            | "poppy"
            | "blue_orchid"
            | "allium"
            | "azure_bluet"
            | "red_tulip"
            | "orange_tulip"
            | "white_tulip"
            | "pink_tulip"
            | "oxeye_daisy"
            | "cornflower"
            | "lily_of_the_valley"
            | "wither_rose"
            | "torchflower"
            | "sunflower"
            | "lilac"
            | "rose_bush"
            | "peony"
            | "pink_petals"
            | "wildflowers"
            | "leaf_litter"
            | "nether_wart"
            | "crimson_roots"
            | "warped_roots"
            | "nether_sprouts"
            | "brown_mushroom"
            | "red_mushroom"
            | "crimson_fungus"
            | "warped_fungus"
    );
    vegetation.then_some(0)
}

/// The block an axe strips this one to (`AxeItem.STRIPPABLES`), keeping its
/// `axis`.
pub fn stripped(name: &str) -> Option<String> {
    let path = name.strip_prefix("minecraft:")?;
    if path.starts_with("stripped_") {
        return None;
    }
    let strippable = ["_log", "_wood", "_stem", "_hyphae"]
        .iter()
        .any(|suffix| path.ends_with(suffix))
        && !path.contains("petrified")
        || path == "bamboo_block";
    strippable.then(|| format!("minecraft:stripped_{path}"))
}

/// What a shovel flattens into a path (`ShovelItem.FLATTENABLES`, and
/// Ultimine's `shovel_flattenable` tag), with air above.
pub fn flattens(name: &str) -> bool {
    matches!(
        name,
        "minecraft:grass_block"
            | "minecraft:dirt"
            | "minecraft:coarse_dirt"
            | "minecraft:podzol"
            | "minecraft:mycelium"
            | "minecraft:rooted_dirt"
    )
}

/// What a hoe tills this block into (`HoeItem.TILLABLES`), with air above.
pub fn tilled(name: &str) -> Option<&'static str> {
    match name {
        "minecraft:grass_block" | "minecraft:dirt" | "minecraft:dirt_path" => {
            Some("minecraft:farmland")
        }
        "minecraft:coarse_dirt" => Some("minecraft:dirt"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stripping_covers_every_species_and_nothing_else() {
        assert_eq!(
            stripped("minecraft:oak_log").as_deref(),
            Some("minecraft:stripped_oak_log")
        );
        assert_eq!(
            stripped("minecraft:warped_hyphae").as_deref(),
            Some("minecraft:stripped_warped_hyphae")
        );
        assert_eq!(
            stripped("minecraft:bamboo_block").as_deref(),
            Some("minecraft:stripped_bamboo_block")
        );
        assert_eq!(stripped("minecraft:stripped_oak_log"), None);
        assert_eq!(stripped("minecraft:stone"), None);
        assert_eq!(stripped("mymod:oak_log"), None);
    }

    #[test]
    fn tilling_and_flattening_follow_the_vanilla_tables() {
        assert_eq!(tilled("minecraft:grass_block"), Some("minecraft:farmland"));
        assert_eq!(tilled("minecraft:coarse_dirt"), Some("minecraft:dirt"));
        assert_eq!(tilled("minecraft:stone"), None);
        assert!(flattens("minecraft:podzol"));
        assert!(!flattens("minecraft:dirt_path"));
    }

    #[test]
    fn plant_kinds_group_like_ultimine() {
        assert_eq!(
            plant_kind("minecraft:wheat"),
            plant_kind("minecraft:carrots")
        );
        assert_eq!(plant_kind("minecraft:oak_sapling"), Some(2));
        assert_eq!(plant_kind("minecraft:cocoa"), Some(3));
        assert_eq!(plant_kind("minecraft:poppy"), Some(0));
        assert_eq!(plant_kind("minecraft:stone"), None);
    }
}
