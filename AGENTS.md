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

## Development Workflow (cubrid-labs org standard)

All non-trivial work across cubrid-labs repositories MUST follow this 5-phase cycle:

1. **Oracle Design Review** — Consult Oracle before implementation to validate architecture, API surface, and approach. Raise concerns early.
2. **Implementation** — Build the feature/fix with tests. Follow existing codebase patterns.
3. **Documentation Update** — Update ALL affected docs (README, CHANGELOG, ROADMAP, API docs, SUPPORT_MATRIX, PRD, etc.) in the same PR or as an immediate follow-up. Code without doc updates is incomplete.
4. **Integration Test against real CUBRID** — Run the feature/fix against a live CUBRID instance via the repo's Docker setup or CI integration matrix. Mock-only validation is INSUFFICIENT. No release may proceed until the relevant integration jobs are green for the target CUBRID/runtime versions.
5. **Oracle Post-Implementation Review** — Consult Oracle to review the completed work for correctness, edge cases, integration coverage, and consistency before merging.

Skipping any phase requires explicit justification recorded in the PR description. Trivial changes (typos, single-line fixes, doc-only edits) may skip phases 1, 4, and 5. Phase 4 is NEVER skippable for any change that touches runtime code paths, including changes that "only" add a new module, a new code path, or a new dependency.

## Validation
- `cargo check --workspace`
- `cargo clippy --workspace -- -D warnings`
- `cargo fmt --all -- --check`
- `cargo test --workspace`
