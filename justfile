set windows-shell := ["powershell"]
set shell := ["bash", "-cu"]

_default:
    just --list -u

ready:
  git diff --exit-code --quiet
  just fmt
  just check
  just test
  just lint
  git status

# INITIALIZE

init-rust:
    cargo binstall rusty-hook taplo-cli cargo-insta cargo-deny -y

init-node:
    yarn install

init:
    just init-rust
    just init-node
    git submodule update --init

# Update our local branch with the remote branch (this is for you to sync the submodules)
update:
    git pull
    git submodule update --init

# CHECKING

check-rust:
    cargo check --workspace

check-node:
    yarn typecheck

check:
    just check-rust
    just check-node

# TESTING

test-rust:
    cargo test --no-fail-fast

test-node:
    yarn build:node
    yarn test

test:
    just test-rust
    just test-node

# FORMATTING

fmt-rust:
    cargo fmt --all -- --emit=files

fmt-lint:
    yarn prettier

fmt:
    just fmt-rust
    just fmt-lint
    taplo format

# LINTING

lint-rust:
    cargo clippy --workspace --all-targets -- --deny warnings
    cargo fmt --all -- --check

lint-node:
    yarn lint-filename
    yarn lint
    yarn prettier:ci

lint:
    just lint-rust
    just lint-node
    taplo format

# smoke commands allow you to run checks locally without pushing branches to the remote
smoke-rust:
    just test-rust
    just check-rust
    just lint-rust

smoke-node:
    just test-node
    just check-node
    just lint-node

smoke:
    just smoke-rust
    just smoke-node


# BENCHING

setup-bench:
    git clone --branch r108 --depth 1 https://github.com/mrdoob/three.js.git ./temp/three
    echo "import * as three from './src/Three.js'; export { three }" > temp/three/entry.js
    mkdir -p temp/three10x
    for i in {1..10}; do cp -r ./temp/three/src ./temp/three10x/copy$i/; done
    echo > temp/three10x/entry.js
    for i in {1..10}; do echo "import * as three$i from './copy$i/Three.js'; export { three$i }" >> temp/three10x/entry.js; done

bench:
    cargo bench -p bench
