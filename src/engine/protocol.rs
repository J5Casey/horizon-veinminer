//! Mod messages between the client and server halves (Horizon's registered
//! payloads, `ultimine:*` ids). Plain text, versioned, size-checked; the
//! server treats everything a client sends as untrusted input.
//!
//! A message is lines of `key=value`, the first `v=<VERSION>`. Lists are
//! comma-separated; each merge group is a line of its own.
use super::config::Rules;
use super::matcher::Group;
use super::shape::ShapeId;

pub const MOD_ID: &str = "ultimine";
/// Client to server: whether Ultimine is active and with which shape.
pub const STATE: &str = "ultimine:state";
/// Server to client: the server's rules, for the preview and the HUD.
pub const RULES: &str = "ultimine:rules";
pub const VERSION: u32 = 2;
/// Our cap on one mod message.
pub const MAX_MESSAGE: usize = 30_000;
/// The cap on a client's state message.
const MAX_STATE: usize = 1024;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClientState {
    pub v: u32,
    pub active: bool,
    pub shape: ShapeId,
    /// Ask for the rules again (a client that missed them).
    pub want_rules: bool,
}

fn flag(b: bool) -> &'static str {
    if b { "1" } else { "0" }
}

fn parse_flag(text: &str) -> Option<bool> {
    match text {
        "1" => Some(true),
        "0" => Some(false),
        _ => None,
    }
}

/// The `key=value` lines of `data`, after a `v=<VERSION>` first line; None
/// for anything else (another version, not UTF-8, too long).
fn lines(data: &[u8], cap: usize) -> Option<Vec<(&str, &str)>> {
    if data.len() > cap {
        return None;
    }
    let text = std::str::from_utf8(data).ok()?;
    let mut out = Vec::new();
    for line in text.lines() {
        out.push(line.split_once('=')?);
    }
    (out.first()? == &("v", VERSION.to_string().as_str())).then(|| out.split_off(1))
}

pub fn encode_state(state: &ClientState) -> Vec<u8> {
    format!(
        "v={}\nactive={}\nshape={}\nwant_rules={}",
        state.v,
        flag(state.active),
        state.shape.key(),
        flag(state.want_rules)
    )
    .into_bytes()
}

pub fn decode_state(data: &[u8]) -> Option<ClientState> {
    let mut state = ClientState {
        v: VERSION,
        ..ClientState::default()
    };
    let (mut active, mut shape) = (None, None);
    for (key, value) in lines(data, MAX_STATE)? {
        match key {
            "active" => active = Some(parse_flag(value)?),
            "shape" => shape = Some(ShapeId::from_key(value)?),
            "want_rules" => state.want_rules = parse_flag(value)?,
            _ => return None,
        }
    }
    state.active = active?;
    state.shape = shape?;
    Some(state)
}

fn list(ids: &[String]) -> String {
    ids.join(",")
}

fn parse_list(text: &str) -> Vec<String> {
    text.split(',')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

fn write_rules(rules: &Rules) -> String {
    let mut out = vec![
        format!("v={VERSION}"),
        format!("max_blocks={}", rules.max_blocks),
        format!("exhaustion_per_block={}", rules.exhaustion_per_block),
        format!("cooldown_ticks={}", rules.cooldown_ticks),
        format!(
            "shapes={}",
            rules
                .shapes
                .iter()
                .map(|s| s.key())
                .collect::<Vec<_>>()
                .join(",")
        ),
        format!("shaped_any_block={}", flag(rules.shaped_any_block)),
        format!("excluded_blocks={}", list(&rules.excluded_blocks)),
        format!("only_blocks={}", list(&rules.only_blocks)),
        format!("require_tool={}", flag(rules.require_tool)),
        format!("drops_to_inventory={}", flag(rules.drops_to_inventory)),
        format!("stop_on_break_fail={}", flag(rules.stop_on_break_fail)),
        format!("right_click_axe={}", flag(rules.right_click_axe)),
        format!("right_click_shovel={}", flag(rules.right_click_shovel)),
        format!("right_click_hoe={}", flag(rules.right_click_hoe)),
        format!("right_click_harvest={}", flag(rules.right_click_harvest)),
    ];
    out.extend(
        rules
            .merge_groups
            .iter()
            .map(|g| format!("group={}", list(g))),
    );
    out.extend(
        rules
            .merge_groups_shaped
            .iter()
            .map(|g| format!("group_shaped={}", list(g))),
    );
    out.join("\n")
}

/// The rules as a message. Over the size cap, the block lists are left out
/// (the preview then falls back to same-block matching, the server still
/// decides with its full rules).
pub fn encode_rules(rules: &Rules) -> Vec<u8> {
    let data = write_rules(rules);
    if data.len() <= MAX_MESSAGE {
        return data.into_bytes();
    }
    let lean = Rules {
        merge_groups: Vec::new(),
        merge_groups_shaped: Vec::new(),
        excluded_blocks: Vec::new(),
        only_blocks: Vec::new(),
        ..rules.clone()
    };
    write_rules(&lean).into_bytes()
}

pub fn decode_rules(data: &[u8]) -> Option<Rules> {
    let mut rules = Rules {
        merge_groups: Vec::new(),
        ..Rules::default()
    };
    let mut groups: Vec<Group> = Vec::new();
    for (key, value) in lines(data, MAX_MESSAGE)? {
        match key {
            "max_blocks" => rules.max_blocks = value.parse().ok()?,
            "exhaustion_per_block" => rules.exhaustion_per_block = value.parse().ok()?,
            "cooldown_ticks" => rules.cooldown_ticks = value.parse().ok()?,
            "shapes" => {
                rules.shapes = value
                    .split(',')
                    .map(ShapeId::from_key)
                    .collect::<Option<_>>()?;
            }
            "shaped_any_block" => rules.shaped_any_block = parse_flag(value)?,
            "excluded_blocks" => rules.excluded_blocks = parse_list(value),
            "only_blocks" => rules.only_blocks = parse_list(value),
            "require_tool" => rules.require_tool = parse_flag(value)?,
            "drops_to_inventory" => rules.drops_to_inventory = parse_flag(value)?,
            "stop_on_break_fail" => rules.stop_on_break_fail = parse_flag(value)?,
            "right_click_axe" => rules.right_click_axe = parse_flag(value)?,
            "right_click_shovel" => rules.right_click_shovel = parse_flag(value)?,
            "right_click_hoe" => rules.right_click_hoe = parse_flag(value)?,
            "right_click_harvest" => rules.right_click_harvest = parse_flag(value)?,
            "group" => groups.push(parse_list(value)),
            "group_shaped" => rules.merge_groups_shaped.push(parse_list(value)),
            _ => return None,
        }
    }
    rules.merge_groups = groups;
    rules.validate().is_ok().then_some(rules)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_round_trip_and_reject_garbage() {
        let state = ClientState {
            v: VERSION,
            active: true,
            shape: ShapeId::LargeTunnel,
            want_rules: false,
        };
        assert_eq!(decode_state(&encode_state(&state)), Some(state));
        assert_eq!(decode_state(b"v=1\nactive=1\nshape=shapeless"), None);
        assert_eq!(decode_state(b"v=2\nactive=1\nshape=cube"), None);
        assert_eq!(decode_state(b"v=2\nactive=1"), None);
        assert_eq!(decode_state(&[b' '; 2000]), None);
        let rules = Rules::default();
        assert_eq!(decode_rules(&encode_rules(&rules)), Some(rules));
        assert_eq!(decode_rules(b"v=2\nmax_blocks=0"), None);
        assert_eq!(decode_rules(b"v=2\nunknown=1"), None);
    }

    #[test]
    fn oversized_rules_drop_their_groups() {
        let rules = Rules {
            merge_groups: vec![(0..4000).map(|i| format!("minecraft:block_{i}")).collect()],
            ..Rules::default()
        };
        let data = encode_rules(&rules);
        assert!(data.len() <= MAX_MESSAGE);
        let sent = decode_rules(&data).unwrap();
        assert!(sent.merge_groups.is_empty());
        assert_eq!(sent.max_blocks, rules.max_blocks);
    }
}
