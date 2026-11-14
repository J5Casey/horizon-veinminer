//! Ultimine's engine: shapes, matching, selection, the client and server
//! state machines and the break loop. It knows nothing of Horizon: the
//! mod crate's glue implements its small traits with the mod API's
//! hooks and handles, and `engine/Cargo.toml` builds these same
//! files on their own so `cargo test` runs without the game.
#![forbid(unsafe_code)]

pub mod blocks;
pub mod client;
pub mod config;
pub mod geom;
pub mod matcher;
pub mod page;
pub mod protocol;
pub mod reentry;
pub mod rightclick;
pub mod select;
pub mod server;
pub mod shape;
