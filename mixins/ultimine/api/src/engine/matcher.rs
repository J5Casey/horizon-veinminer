//! Which blocks count as "the same" as the one broken. As in Ultimine, the
//! matcher is chosen once from the broken block and the shape: a block in a
//! merge group matches its group, a plant matches plants of its kind, and
//! anything else matches only itself. Shaped mining may instead take any
//! block (Ultimine's `merge_tags_shaped = ["*"]`, its default).
use super::blocks;
use super::config::Rules;

/// Answers block tag membership (`#minecraft:logs`). The server answers from
/// its registries; a client preview has no tags and answers `None`, which
/// counts as "not in the tag".
pub trait Tags {
    fn in_tag(&self, block: &str, tag: &str) -> Option<bool>;
}

/// No tags known: groups match by their explicit block names only.
pub struct NoTags;
impl Tags for NoTags {
    fn in_tag(&self, _: &str, _: &str) -> Option<bool> {
        None
    }
}

/// A merge group: block ids (`minecraft:iron_ore`) and block tags
/// (`#c:ores/iron`) whose members mine together.
pub type Group = Vec<String>;

fn member(group: &Group, block: &str, tags: &dyn Tags) -> bool {
    group.iter().any(|entry| match entry.strip_prefix('#') {
        Some(tag) => tags.in_tag(block, tag) == Some(true),
        None => entry == block,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchKind {
    SameBlock,
    Groups,
    AnyBlock,
    Plant,
}

pub struct Matcher<'a> {
    pub kind: MatchKind,
    origin: String,
    groups: Vec<&'a Group>,
    tags: &'a dyn Tags,
    rules: &'a Rules,
}

impl<'a> Matcher<'a> {
    /// The matcher for breaking `origin` with a shape that is `shapeless`
    /// (indeterminate) or not.
    pub fn choose(rules: &'a Rules, tags: &'a dyn Tags, origin: &str, shapeless: bool) -> Self {
        let source = if shapeless {
            &rules.merge_groups
        } else {
            &rules.merge_groups_shaped
        };
        let groups: Vec<&Group> = source.iter().filter(|g| member(g, origin, tags)).collect();
        let kind = if !shapeless && rules.shaped_any_block {
            MatchKind::AnyBlock
        } else if !groups.is_empty() {
            MatchKind::Groups
        } else if blocks::plant_kind(origin).is_some() {
            MatchKind::Plant
        } else {
            MatchKind::SameBlock
        };
        Matcher {
            kind,
            origin: origin.to_owned(),
            groups,
            tags,
            rules,
        }
    }

    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// Whether the block named `name` joins the selection.
    pub fn matches(&self, name: &str) -> bool {
        if blocks::is_air(name)
            || blocks::is_unbreakable(name)
            || self.rules.excluded_blocks.iter().any(|b| b == name)
            || !self.rules.only_blocks.is_empty()
                && !self.rules.only_blocks.iter().any(|b| b == name)
        {
            return false;
        }
        if name == self.origin {
            return true;
        }
        match self.kind {
            MatchKind::SameBlock => false,
            MatchKind::Groups => self.groups.iter().any(|g| member(g, name, self.tags)),
            MatchKind::Plant => {
                blocks::plant_kind(name).is_some()
                    && blocks::plant_kind(name) == blocks::plant_kind(&self.origin)
            }
            MatchKind::AnyBlock => !blocks::is_fluid(name) && !blocks::is_hard(name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct LogTag;
    impl Tags for LogTag {
        fn in_tag(&self, block: &str, tag: &str) -> Option<bool> {
            Some(tag == "minecraft:logs" && block.ends_with("_log"))
        }
    }

    #[test]
    fn shapeless_merges_groups_and_otherwise_matches_itself() {
        let rules = Rules::default();
        let m = Matcher::choose(&rules, &NoTags, "minecraft:iron_ore", true);
        assert_eq!(m.kind, MatchKind::Groups);
        assert!(m.matches("minecraft:deepslate_iron_ore"));
        assert!(!m.matches("minecraft:gold_ore"));
        let m = Matcher::choose(&rules, &NoTags, "minecraft:glass", true);
        assert_eq!(m.kind, MatchKind::SameBlock);
        assert!(m.matches("minecraft:glass"));
        assert!(!m.matches("minecraft:stone"));
    }

    #[test]
    fn shaped_any_block_skips_air_fluids_hard_and_unbreakable_blocks() {
        let rules = Rules::default();
        let m = Matcher::choose(&rules, &NoTags, "minecraft:stone", false);
        assert_eq!(m.kind, MatchKind::AnyBlock);
        assert!(m.matches("minecraft:dirt"));
        for name in [
            "minecraft:air",
            "minecraft:water",
            "minecraft:obsidian",
            "minecraft:bedrock",
        ] {
            assert!(!m.matches(name), "{name}");
        }
        let m = Matcher::choose(&rules, &NoTags, "minecraft:obsidian", false);
        assert!(m.matches("minecraft:obsidian"));
    }

    #[test]
    fn same_block_shaped_mode_and_tag_groups() {
        let rules = Rules {
            shaped_any_block: false,
            merge_groups_shaped: vec![vec!["#minecraft:logs".into()]],
            ..Rules::default()
        };
        let m = Matcher::choose(&rules, &LogTag, "minecraft:oak_log", false);
        assert_eq!(m.kind, MatchKind::Groups);
        assert!(m.matches("minecraft:birch_log"));
        let m = Matcher::choose(&rules, &NoTags, "minecraft:oak_log", false);
        assert_eq!(m.kind, MatchKind::SameBlock);
        assert!(!m.matches("minecraft:birch_log"));
    }

    #[test]
    fn plants_match_their_kind_and_exclusions_always_win() {
        let rules = Rules {
            excluded_blocks: vec!["minecraft:carrots".into()],
            ..Rules::default()
        };
        let m = Matcher::choose(&rules, &NoTags, "minecraft:wheat", true);
        assert_eq!(m.kind, MatchKind::Plant);
        assert!(m.matches("minecraft:potatoes"));
        assert!(!m.matches("minecraft:carrots"));
        assert!(!m.matches("minecraft:oak_sapling"));
    }
}
