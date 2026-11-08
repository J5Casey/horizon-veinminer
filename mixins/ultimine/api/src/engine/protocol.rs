//! Mod messages between the client and server halves (Horizon's mod
//! messages, `mod_id:name` channels). JSON, versioned, size-checked; the
//! server treats everything a client sends as untrusted input.
use super::config::Rules;
use super::shape::ShapeId;
use serde::{Deserialize, Serialize};

pub const MOD_ID: &str = "ultimine";
/// Client to server: whether Ultimine is active and with which shape.
pub const STATE: &str = "ultimine:state";
/// Server to client: the server's rules, for the preview and the HUD.
pub const RULES: &str = "ultimine:rules";
pub const VERSION: u32 = 1;
/// Horizon's cap on one mod message.
pub const MAX_MESSAGE: usize = 30_000;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClientState {
    pub v: u32,
    pub active: bool,
    pub shape: ShapeId,
    /// Ask for the rules again (a client that missed them).
    #[serde(default)]
    pub want_rules: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RulesMessage {
    pub v: u32,
    pub rules: Rules,
}

pub fn encode_state(state: &ClientState) -> Vec<u8> {
    serde_json::to_vec(state).unwrap_or_default()
}

pub fn decode_state(data: &[u8]) -> Option<ClientState> {
    if data.len() > 1024 {
        return None;
    }
    let state: ClientState = serde_json::from_slice(data).ok()?;
    (state.v == VERSION).then_some(state)
}

/// The rules as a message. Over the size cap, the merge groups are left out
/// (the preview then falls back to same-block matching, the server still
/// decides with its full rules).
pub fn encode_rules(rules: &Rules) -> Vec<u8> {
    let mut message = RulesMessage {
        v: VERSION,
        rules: rules.clone(),
    };
    let data = serde_json::to_vec(&message).unwrap_or_default();
    if data.len() <= MAX_MESSAGE {
        return data;
    }
    message.rules.merge_groups.clear();
    message.rules.merge_groups_shaped.clear();
    message.rules.excluded_blocks.clear();
    message.rules.only_blocks.clear();
    serde_json::to_vec(&message).unwrap_or_default()
}

pub fn decode_rules(data: &[u8]) -> Option<Rules> {
    if data.len() > MAX_MESSAGE {
        return None;
    }
    let message: RulesMessage = serde_json::from_slice(data).ok()?;
    (message.v == VERSION && message.rules.validate().is_ok()).then_some(message.rules)
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
        assert_eq!(
            decode_state(br#"{"v":2,"active":true,"shape":"shapeless"}"#),
            None
        );
        assert_eq!(
            decode_state(br#"{"v":1,"active":true,"shape":"cube"}"#),
            None
        );
        assert_eq!(decode_state(&[b' '; 2000]), None);
        let rules = Rules::default();
        assert_eq!(decode_rules(&encode_rules(&rules)), Some(rules));
        assert_eq!(decode_rules(br#"{"v":1,"rules":{"max_blocks":0}}"#), None);
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
