set windows-shell := ["powershell"]
set shell := ["bash", "-cu"]

_default:
    just --list -u

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

# Initialize the project and its submodules
init:
    just init-rust
    just init-node
    git submodule update --init

init-rust:
    cargo binstall taplo-cli cargo-insta cargo-deny -y

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

check:
    just check-rust
    just check-node

# run tests for both Rust and Node.js
test:
    just test-rust
    just test-node

test-rust:
    cargo test --no-fail-fast

test-node:
    pnpm build
    pnpm test

# Fix formatting issues both for Rust, Node.js and all files in the repository
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
    node ./scripts/misc/setup-benchmark-input.js

bench:
    cargo bench -p bench

# RELEASING

bump packages *args: 
  node ./scripts/misc/bump-version.js {{args}}

changelog:
  pnpm conventional-changelog --preset angular --i CHANGELOG.md --same-file --pkg=./packages/rolldown/package.json
