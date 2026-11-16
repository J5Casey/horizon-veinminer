# Working on this mod

Read README and docs/ first. The mod must use only the game's catalog hooks and
`Host` helpers (no injections into private functions). Keep gameplay logic in
`src/engine/` (no game types) with tests in `tests/` or
the engine's own test modules; keep the glue thin. Run cargo fmt, clippy with
warnings denied, `cargo test` (in `engine/`) for relevant changes; clippy the mod against
the SDK (`.sunrise/sdk/api`, see Cargo.toml). Keep docs/compatibility.md honest.
