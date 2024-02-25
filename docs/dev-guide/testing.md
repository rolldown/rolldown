# Testing

In general, we have two main test suites for rust and node.js.

## Summary

- `just test-rust` for running all rust tests.
- `just test-node` for running all node tests.

## Rust Testing

Tests cases are stored in

- `/crates/rolldown/tests/esbuild`
- `/crates/rolldown/tests/fixures`

### Test Case

A test case is a folder that contains `test.config.json`.

`test.config.json` contains the configuration for the test suite. See https://github.com/rolldown-rs/rolldown/blob/main/crates/rolldown_testing/src/test_config/mod.rs for more information.

- `main.js` is the default entry of the test case, if `input.input` is not specified in `test.config.json`.

Rolldown will bundle the input into `/dist`, and using the same `node` instance to execute every entry file in `/dist` orderly. If `_test.mjs` is found in test case folder, it will be executed after all entry points are executed.

## Node Testing

### Building Bindings

For the NAPI-RS based packages to work, and for tests to run, they must be built first. This is done by running `yarn build` in the root directory. This will spin up a process that builds the Node/WASM binding crates (with Cargo), and then builds the rolldown npm package. The `yarn build` script is also smart enough to only re-build if it detects changes since the last time it was ran.

Further, the `yarn watch` script can be ran that will watch the file system for changes, and re-build the bindings and npm package when changes are detected (using the same process as `yarn build`). This is useful for development when you're constantly making changes and re-running tests.

### Rolldown Testing

The rolldown testing is located at `packages/node/test`. It is used to test rolldown exported node api, also including options. If you add a options from rollup, please add corresponding test case for it.

In `/packages/node`,

- `yarn test` will run rollup tests.
- `yarn test:update` will run and update the tests status.

### Rollup Testing

Tests cases are stored in `/rollup`, which is a git submodule of `rolldown`. So you need to run `just update` to initialize the submodule before running these tests.

In `/packages/rollup-tests`,

- `yarn test` will run rollup tests.
- `yarn test:update` will run and update the tests status.
