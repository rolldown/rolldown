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
    pnpm install



# Update our local branch with the remote branch (this is for you to sync the submodules)
update:
    git pull
    git submodule update --init

# CHECKING

check-rust:
    cargo check --workspace

check-node:
    pnpm type-check

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
    pnpm build
    pnpm test

# Fix formatting issues both for Rust and Node.js.
fmt:
    just fmt-rust
    just fmt-repo

fmt-rust:
    cargo fmt --all -- --emit=files
    taplo fmt

fmt-repo:
    pnpm lint-prettier:fix
    pnpm lint-toml:fix

# lint the codebase
lint:
    just lint-rust
    just lint-node

lint-rust:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- --deny warnings

lint-node:
    pnpm lint-code

lint-repo:
    pnpm lint-filename
    pnpm lint-prettier
    pnpm lint-spell
    pnpm lint-toml


# BENCHING

setup-bench:
    node ./scripts/setup-rust-benchmark-input.js

bench:
    cargo bench -p bench

# RELEASING

change:
    pnpm changeset add

no-change:
    pnpm changeset add --empty

version:
    pnpm changeset version

publish:
    pnpm changeset publish
    git push --follow-tags
