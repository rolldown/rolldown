set windows-shell := ["powershell"]
set shell := ["bash", "-cu"]

_default:
    just --list -u

# `smoke` command almost run all ci checks locally. It's useful to run this before pushing your changes.
smoke:
    just smoke-rust
    just smoke-node

smoke-rust:
    just check-rust
    just test-rust
    just lint-rust

smoke-node:
    just check-node
    just test-node
    just lint-node

# Initialize the project and its submodules
init:
    just init-rust
    just init-node
    git submodule update --init

init-rust:
    cargo binstall taplo-cli cargo-insta cargo-deny typos-cli -y

init-node:
    yarn install



# Update our local branch with the remote branch (this is for you to sync the submodules)
update:
    git pull
    git submodule update --init

# CHECKING

check-rust:
    cargo check --workspace

check-node:
    yarn type-check

check-typo:
    typos

check:
    just check-rust
    just check-node
    just check-typo

# run tests for both Rust and Node.js
test:
    just test-rust
    just test-node

test-rust:
    cargo test --no-fail-fast

test-node:
    yarn build
    yarn test

# Fix formatting issues both for Rust and Node.js.
fmt:
    just fmt-rust
    just fmt-lint
  
fmt-rust:
    cargo fmt --all -- --emit=files
    taplo fmt

fmt-lint:
    yarn prettier

# lint the codebase
lint:
    just lint-rust
    just lint-node

lint-rust:
    cargo fmt --all -- --check
    taplo fmt --check
    cargo clippy --workspace --all-targets -- --deny warnings

lint-node:
    yarn lint-filename
    yarn lint-code
    yarn prettier:ci

# BENCHING

setup-bench:
    node ./scripts/setup-rust-benchmark-input.js

bench:
    cargo bench -p bench
