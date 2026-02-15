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
#   2. Rebuild to verify + embed new version
#   3. Commit, tag, push
#
# Usage: make release V=0.1.1
release:
ifndef V
	$(error Usage: make release V=x.y.z)
endif
	@echo "==> Bumping version to $(V)"
	sed -i '' 's/^version = ".*"/version = "$(V)"/' Cargo.toml
	@echo "==> Building release"
	cargo build --release
	@echo "==> Running tests"
	cargo test
	@echo "==> Version check"
	./target/release/libretto --version
	@echo "==> Committing and tagging v$(V)"
	git add -A
	git commit -m "Release v$(V)"
	git tag "v$(V)"
	@echo "==> Pushing to origin"
	git push
	git push origin "v$(V)"
	@echo "==> Released v$(V)"
