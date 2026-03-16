# AGENTS.md

## Purpose
`cubrid-rs` provides native Rust database drivers for CUBRID - sync (`cubrid-client`) and async (`cubrid-tokio`) - implementing the CAS wire protocol directly, with no FFI or C dependencies.

## Read First
- `README.md`
- `docs/PRD.md`
- `docs/ARCHITECTURE.md`
- `docs/PROTOCOL_RESEARCH.md`
- `CONTRIBUTING.md`

## Working Rules
- All crates must compile with `#![deny(unsafe_code)]` - zero unsafe.
- Wire protocol details live in `cubrid-protocol`; clients depend on it, never the reverse.
- Keep public API surface minimal - expose only what users need.
- All protocol parsing must handle malformed input gracefully (no panics).
- Prefer `thiserror` for error types, never `anyhow` in library code.
- Tests go in `tests/` (integration) or inline `#[cfg(test)]` modules (unit).
- Match the ecosystem patterns from other cubrid-labs repos (README structure, badges, labels, workflows).

## Validation
- `cargo check --workspace`
- `cargo clippy --workspace -- -D warnings`
- `cargo fmt --all -- --check`
- `cargo test --workspace`
