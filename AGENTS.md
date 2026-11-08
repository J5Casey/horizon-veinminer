# Working on this mod

Read README and docs/ first. The mod must use only Horizon's catalog hooks and
`Host` helpers (no injections into private functions). Keep gameplay logic in
`mixins/ultimine/api/src/engine/` (no Horizon types) with tests in `tests/` or
the engine's own test modules; keep the glue thin. Run cargo fmt, clippy with
warnings denied, cargo test, `tools/test_package.py` and `tools/check_api.py`
(with `--clippy`) for relevant changes. Keep docs/compatibility.md honest.
