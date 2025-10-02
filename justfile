set windows-shell := ["powershell"]
set shell := ["bash", "-cu"]

alias dt := t-run

_default:
  just --list -u

setup:
  just check-setup-prerequisites
  # Rust related setup
  cargo install cargo-binstall
  cargo binstall cargo-insta cargo-deny cargo-shear typos-cli -y
  # Node.js related setup
  corepack enable
  pnpm install
  just setup-submodule
  just setup-bench
  @echo "✅✅✅ Setup complete!"

setup-submodule:
  git submodule update --init

setup-bench:
  node --import @oxc-node/core/register ./scripts/misc/setup-benchmark-input/index.js

# Update the submodule to the latest commit
update-submodule:
  git submodule update --init

# --- `roll` series commands will run all relevant commands in one go.

# Run all relevant commands.
roll: pnpm-install roll-rust roll-node roll-repo

# Run all relevant commands for Rust.
roll-rust: pnpm-install test-rust lint-rust

# Run all relevant commands for Node.js.
roll-node: test-node lint-node

# Run all relevant commands for the repository.
roll-repo: lint-repo

# --- `test` series commands aim to run tests and update snapshots automatically.
test: test-rust test-node update-generated-code

# Update snapshots both for Rust and Node.js tests.
test-update:
  just test-rust # Rust tests will update snapshots automatically.
  just test-node --update

# Update snapshots for Node.js tests.
test-update-node:
  just test-node --update

# Run Rust tests.
test-rust: pnpm-install
  cargo test --workspace --exclude rolldown_binding

# Run Node.js tests for Rolldown.
test-node-rolldown *args="": build-rolldown
  just t-node-rolldown {{ args }}

# Run Node.js tests for Rolldown without building Rolldown.
# This command is still useful until we have advanced caching util.
test-node-rolldown-only  *args="":
  just t-node-rolldown {{ args }}

# Run Rollup's test suite to check Rolldown's behaviors.
test-node-rollup *args="": build-rolldown
  just t-node-rollup {{ args }}

# Run both Rolldown's tests and Rollup's test suite.
test-node *args="": build-rolldown
  just test-node-rolldown {{ args }}
  just test-node-rollup

test-node-hmr *args: build build-test-dev-server
  just test-node-hmr-only {{ args }}

test-node-hmr-only *args:
  pnpm run --filter @rolldown/test-dev-server-tests test {{ args }}

# Run Vite's test suite to check Rolldown's behaviors.
test-vite: # We don't use `test-node-vite` because it's not expected to run in `just test-node`.
  pnpm run --filter vite-tests test

# --- `t` series commands provide scenario-specific shortcut commands for testing compared to `test` series commands.

# Run both Rolldown's tests and Rollup's test suite without building Rolldown.
t-node: t-node-rolldown t-node-rollup

# Run Rolldown's tests without building Rolldown.
t-node-rolldown *args="":
  pnpm run --filter rolldown-tests test:main {{ args }}
  pnpm run --filter rolldown-tests test:watcher {{ args }}

# Run Rollup's test suite without building Rolldown.
t-node-rollup command="":
  pnpm run --filter rollup-tests test{{ command }}

# Run specific rust test without enabling extended tests.
t-run *args:
  NEEDS_EXTENDED=false cargo run-fixture {{ args }}

# --- `fix` series commands aim to fix fixable issues.

# Fix formatting issues both for Rust, Node.js and all files in the repository
fix: fix-rust fix-node fix-repo

# Fix formatting, linting and code fixing issues for Rust files.
fix-rust:
  cargo fmt --all -- --emit=files
  -cargo shear --fix # omit exit status with `-`
  cargo fix --allow-dirty --allow-staged

# Fix linting issues for Node.js files.
fix-node:
  pnpm lint-code -- --fix

# Fix formatting issues for all files except Rust files.
fix-repo:
  pnpm run fmt


# --- `lint` series commands aim to catch linting and type checking issues.

lint: lint-rust lint-node lint-repo

# Linting formatting, syntax and linting issues for Rust files.
lint-rust: clippy
  cargo fmt --all --check
  cargo check --workspace --all-features --all-targets --locked

# For the most of the time, code is automatically formatted on save in the editor.
# Also, clippy already cover compiler error.
clippy:
  cargo clippy --workspace --all-targets -- --deny warnings


lint-node:
  pnpm lint-code
  pnpm type-check
  pnpm lint-knip

lint-repo:
  typos # Check if the spelling is correct.
  cargo ls-lint # Check if the file names are correct.
  dprint check # Check if files are formatted correctly.

# --- `build` series commands aim to provide a easy way to build the project.



# Build both `@rolldown/pluginutils` and rolldown
build: build-pluginutils build-rolldown

# Build `@rolldown/debug` located in `packages/debug`.
build-rolldown-debug:
  pnpm run --filter "@rolldown/debug" build

# Only build `rolldown` located in `packages/rolldown` itself without triggering building binding `crates/rolldown_binding`.
build-glue:
  pnpm run --filter rolldown build-js-glue

# Only build `.node` binding located in `packages/rolldown`.
build-rolldown-binding:
  pnpm run --filter rolldown build-binding

# Build `rolldown` located in `packages/rolldown` itself and its `.node` binding.
build-rolldown: build-pluginutils
  pnpm run --filter rolldown build-native:debug

# Build `rolldown` located in `packages/rolldown` itself and its `.wasm` binding for WASI.
build-rolldown-wasi: build-pluginutils
  pnpm run --filter rolldown build-wasi:debug

# Build `rolldown` located in `packages/rolldown` itself and its `.node` binding in release mode.
build-rolldown-release: build-pluginutils
  pnpm run --filter rolldown build-native:release

# Build `rolldown` located in `packages/rolldown` itself and its `.node` binding in profile mode.
build-rolldown-profile:
  pnpm run --filter rolldown build-native:profile

build-rolldown-memory-profile:
  pnpm run --filter rolldown build-native:memory-profile

# Build `@rolldown/browser` located in `packages/browser` itself and its `.wasm` binding.
build-browser: build-pluginutils
  pnpm run --filter "@rolldown/browser" build:debug

# Build `@rolldown/browser` located in `packages/browser` itself and its `.wasm` binding in release mode.
build-browser-release: build-pluginutils
  pnpm run --filter "@rolldown/browser" build:release

# Build `@rolldown/pluginutils` located in `packages/pluginutils`.
build-pluginutils:
  pnpm run --filter "@rolldown/pluginutils" build

# Build `@rolldown/test-dev-server` located in `packages/test-dev-server`.
build-test-dev-server:
  pnpm run --filter @rolldown/test-dev-server build

# --- `bench` series commands aim to provide a easy way to run benchmarks.

bench-rust:
  cargo bench -p bench

bench-node:
  pnpm --filter bench run bench

bench-node-par:
  pnpm --filter bench exec node ./benches/par.js


# --- Misc

bump-packages *args:
  node --import @oxc-node/core/register ./scripts/misc/bump-version.js {{ args }}

check-setup-prerequisites:
  node ./scripts/misc/setup-prerequisites/node.js

# Trigger pnpm install. This is used the ensure up-to-date dependencies before running any commands.
pnpm-install:
  pnpm install

# Regenerate auto-generated code files from templates (must run after core changes).
# This generates:
# - Runtime helper definitions (crates/rolldown_common/src/generated/runtime_helper.rs)
# - Check options (crates/rolldown_common/src/generated/checks_options.rs + TypeScript equivalents)
# - Hook usage tracking (crates/rolldown_plugin/src/generated/hook_usage.rs + TypeScript equivalent)
# - Event kind switching logic (crates/rolldown_error/src/generated/event_kind_switcher.rs)
update-generated-code:
  cargo run --bin generator

# Run the `rolldown` cli using node.
run *args:
  pnpm rolldown {{ args }}
  
