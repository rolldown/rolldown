set windows-shell := ["powershell"]
set shell := ["bash", "-cu"]

_default:
    just --list -u

setup:
    # Rust related setup
    cargo install cargo-binstall
    cargo binstall taplo-cli cargo-insta cargo-deny cargo-shear -y
    # Node.js related setup
    corepack enable
    pnpm install
    just setup-submodule
    just setup-bench
    @echo "✅✅✅ Setup complete!"

setup-submodule:
    git submodule update --init

setup-bench:
    node ./scripts/misc/setup-benchmark-input/index.js

# Update the submodule to the latest commit
update-submodule:
    git submodule update --init

# `roll` command almost run all ci checks locally. It's useful to run this before pushing your changes.

roll:
    just roll-rust
    just roll-node
    just roll-repo

roll-rust:
    just check-rust
    just test-rust
    just lint-rust

roll-node:
    just test-node
    just check-node
    just lint-node

roll-repo:
    just lint-repo

# CHECKING

check: check-rust check-node

check-rust:
    cargo check --workspace

check-node:
    pnpm type-check

# run tests for both Rust and Node.js
test: test-rust test-node

test-rust:
    cargo test --no-fail-fast

# Supported presets: all, rolldown, rollup
test-node preset="all" *args="": _build-native-debug
    just _test-node-{{ preset }} {{ args }}

test-node-only preset="all" *args="":
    just _test-node-{{ preset }} {{ args }}

_test-node-all:
    pnpm run --recursive --parallel --filter=!rollup-tests test 
    # We run rollup tests separately to have a clean output.
    pnpm run --filter rollup-tests test

_test-node-rolldown *args:
    pnpm run --filter rolldown test {{ args }}

_test-node-rollup command="":
    pnpm run --filter rollup-tests test{{ command }}

# Fix formatting issues both for Rust, Node.js and all files in the repository

fmt: fmt-rust fmt-repo

fmt-rust:
    cargo fmt --all -- --emit=files
    taplo fmt
    cargo shear --fix

fmt-repo:
    pnpm lint-prettier:fix
    pnpm lint-toml:fix

# lint the codebase
lint: lint-rust lint-node lint-repo

lint-rust:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- --deny warnings
    cargo shear

lint-node:
    pnpm lint-code

lint-repo:
    pnpm lint-repo

fix: fix-rust fix-repo

fix-rust:
    just fmt-rust
    cargo fix --allow-dirty
    cargo shear --fix

fix-repo:
    pnpm lint-code --fix
    just fmt-repo

# Support `just build [native|wasi] [debug|release]`
build target="native" mode="debug":
    pnpm run --filter rolldown build-{{ target }}:{{ mode }}

_build-native-debug:
    just build native debug

# BENCHING

bench-rust:
    cargo bench -p bench

bench-node:
    pnpm --filter bench run bench 

bench-node-par:
    pnpm --filter bench exec node ./benches/par.js

# RELEASING

bump packages *args:
    node ./scripts/misc/bump-version.js {{ args }}

changelog:
    pnpm conventional-changelog --preset angular --i CHANGELOG.md --same-file --pkg=./packages/rolldown/package.json

