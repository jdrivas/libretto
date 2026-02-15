# Libretto project automation
#
# Usage:
#   make build          Build release binary
#   make test           Run all tests
#   make check          Lint + test (CI-style)
#   make release V=0.1.1  Bump version, commit, tag, push

.PHONY: build test check clean release

# Default target
build:
	cargo build --release

test:
	cargo test

check: test
	cargo clippy -- -D warnings

clean:
	cargo clean

# Release workflow:
#   1. Bump version in workspace Cargo.toml
#   2. Run tests to verify
#   3. Commit and tag (so the tree is clean)
#   4. Build release binary (embeds clean git hash)
#   5. Push
#
# Usage: make release V=0.1.1
release:
ifndef V
	$(error Usage: make release V=x.y.z)
endif
	@echo "==> Bumping version to $(V)"
	sed -i '' 's/^version = ".*"/version = "$(V)"/' Cargo.toml
	@echo "==> Running tests"
	cargo test
	@echo "==> Committing and tagging v$(V)"
	git add -A
	git commit -m "Release v$(V)"
	git tag "v$(V)"
	@echo "==> Building release (clean hash)"
	cargo build --release
	@echo "==> Version check"
	./target/release/libretto --version
	@echo "==> Pushing to origin"
	git push
	git push origin "v$(V)"
	@echo "==> Released v$(V)"
