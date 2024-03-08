# Testing

We have two groups of test suites: one for Rust, and one for Node.js.

## Summary

- `just test-rust` for running all Rust tests.
- `just test-node` for running all Node.js tests.

## Rust Tests

Rust tests cases are stored in

- `/crates/rolldown/tests/esbuild`
- `/crates/rolldown/tests/fixures`

### How test cases are defined

A test case is a folder that contains `test.config.json`.

`test.config.json` contains the configuration for the test suite. See https://github.com/rolldown-rs/rolldown/blob/main/crates/rolldown_testing/src/test_config/mod.rs for more information.

- `main.js` is the default entry of the test case, if `input.input` is not specified in `test.config.json`.

Rolldown will bundle the input into `/dist`, and using the same `node` instance to execute every entry file in `/dist` orderly. If `_test.mjs` is found in test case folder, it will be executed after all entry points are executed.

## Node.js Tests

:::tip
Make sure to [build Node.js bindings](./build.md) before running these tests.
:::

### Node.js API tests

Tests located in `packages/node/test` are used to test Rolldown's Node.js API (i.e. the API of the `@rolldown/node` package published on NPM).

It is our goal to align Rolldown's Node.js API with that of Rollup's as much as possible, and the tests are used to verify API alignment and track the progress. Currently, there are many Rollup options that are not yet supported. If you implemented support for additional options from rollup, please add corresponding test cases for them.

In `/packages/node`:

- `yarn test` will run rollup tests.
- `yarn test:update` will run and update the tests' status.

### Rollup behavior alignment tests

We also aim for behavior alignment with Rollup by running Rollup's own tests against Rolldown.

To achieve this, each test case in `packages/rollup-tests/test` proxies to the corresponding test in the `rollup` git submodule in project root.

The git submodule should have been initialized after running `just init` when setting up the project, but you should also run `just update` to update the submodule before running the Rollup tests.

In `/packages/rollup-tests`:

- `yarn test` will run rollup tests.
- `yarn test:update` will run and update the tests' status.
