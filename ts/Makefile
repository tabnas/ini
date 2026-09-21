.PHONY: all build test clean embed build-ts build-go test-ts test-go clean-ts clean-go publish-go tags-go tidy-go reset build-rs test-rs clean-rs

all: build test

build: build-ts build-go build-rs

test: test-ts test-go test-rs

clean: clean-ts clean-go clean-rs

# The embedded grammar: ini-grammar.jsonic into ts/src/ini.ts,
# go/ini.go and rs/src/lib.rs.
embed:
	node embed-grammar.js

# TypeScript
build-ts:
	npm run build

test-ts:
	npm test

clean-ts:
	rm -rf dist dist-test

# Go
build-go:
	cd ../go && go build ./...

test-go:
	cd ../go && go test ./...

clean-go:
	cd ../go && go clean -cache

# Rust (the crate in ../rs; the engine is a sibling checkout)
#
# `embed` first, as npm's own `build` script gives build-ts.
# rs/src/lib.rs carries the grammar as GRAMMAR_TEXT, the same jsonic
# text the other two embed, and the crate parses it at load; so without
# this, a focused build after an edit to ini-grammar.jsonic compiles the
# previous text and succeeds.
build-rs: embed
	cd ../rs && cargo build --all-targets

# `--all-targets` does NOT include doctests, and the README's examples
# ARE doctests of the crate, so both lines are needed.
test-rs:
	cd ../rs && cargo test --all-targets && cargo test --doc
	cd ../rs && cargo clippy --all-targets --all-features -- -D warnings

clean-rs:
	cd ../rs && cargo clean

# Publish Go module: make publish-go V=0.1.7
publish-go: test-go
	@test -n "$(V)" || (echo "Usage: make publish-go V=x.y.z" && exit 1)
	@grep -q '^const VERSION = ' ../go/ini.go || (echo "publish-go: no 'const VERSION = ' in ../go/ini.go; refusing to release with an unbumped constant" && exit 1)
	sed -i '' 's/^const VERSION = ".*"/const VERSION = "$(V)"/' ../go/ini.go
	git add ../go/ini.go
	git commit -m "go: v$(V)"
	git tag go/v$(V)
	git push origin main go/v$(V)
	if command -v gh >/dev/null 2>&1; then gh release create go/v$(V) --title "go/v$(V)" --notes "Go module release v$(V)"; fi

tidy-go:
	cd ../go && go mod tidy

tags-go:
	git tag -l 'go/v*' --sort=-version:refname

reset:
	npm run reset
	cd ../go && go clean -cache
	cd ../go && go build ./...
	cd ../go && go test -v ./...
