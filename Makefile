.PHONY: build test lint fmt check clippy audit clean integration changelog doctor

# Build all crates
build:
	cargo build --workspace

# Run all tests
test:
	cargo test --workspace

# Run tests with coverage (requires cargo-tarpaulin)
test-cov:
	@which cargo-tarpaulin > /dev/null 2>&1 && cargo tarpaulin --workspace --out html || echo "cargo-tarpaulin not installed (cargo install cargo-tarpaulin)"

# Run integration tests (requires CUBRID)
integration:
	cargo test --workspace -- --ignored

# Check formatting
fmt:
	cargo fmt --all

# Check formatting (CI mode)
fmt-check:
	cargo fmt --all -- --check

# Run clippy
clippy:
	cargo clippy --workspace -- -D warnings

# Full lint check
lint: fmt-check clippy

# Full check (format + clippy + test)
check: lint test

# Security audit
audit:
	@which cargo-audit > /dev/null 2>&1 && cargo audit || echo "cargo-audit not installed (cargo install cargo-audit)"

# Clean build artifacts
clean:
	cargo clean

# Generate changelog (requires git-cliff)
changelog:
	@which git-cliff > /dev/null 2>&1 && git-cliff -o CHANGELOG.md || echo "git-cliff not installed, skipping"

# Doctor check
doctor:
	@echo "=== Rust Environment ==="
	@rustc --version
	@cargo --version
	@echo ""
	@echo "=== Tools ==="
	@which cargo-tarpaulin > /dev/null 2>&1 && echo "✓ cargo-tarpaulin" || echo "✗ cargo-tarpaulin (optional: cargo install cargo-tarpaulin)"
	@which cargo-audit > /dev/null 2>&1 && echo "✓ cargo-audit" || echo "✗ cargo-audit (optional: cargo install cargo-audit)"
	@which git-cliff > /dev/null 2>&1 && echo "✓ git-cliff" || echo "✗ git-cliff (optional)"
