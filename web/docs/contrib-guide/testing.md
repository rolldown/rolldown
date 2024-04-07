# Testing

We have two groups of test suites: one for Rust, and one for Node.js.

## Summary

- `just test-rust` for running all Rust tests.
- `just test-node` for running all Node.js tests.

## Rust Tests

Rust tests cases are stored in

- `/crates/rolldown/tests/esbuild`
- `/crates/rolldown/tests/fixtures`

### How test cases are defined

A test case is a folder that contains `_config.json`.

`_config.json` contains the configuration for the test suite. If everything works right, you should be able to have auto-completion while editing `_config.json` due to the [config](https://github.com/rolldown/rolldown/blob/226df5b9993e568ff58a345ef1ca39d2cdc37613/.vscode/settings.json#L36-L40).

For all available options, you could refer to

- https://github.com/rolldown/rolldown/blob/main/crates/rolldown_testing/src/test_config/mod.rs
- https://github.com/rolldown/rolldown/blob/main/crates/rolldown_common/src/inner_bundler_options/mod.rs
- https://github.com/rolldown/rolldown/blob/main/crates/rolldown_testing/_test.scheme.json

- `main.js` is the default entry of the test case, if `input.input` is not specified in `_test.json`.

Rolldown will bundle the input into `/dist`, and using the same `node` instance to execute every entry file in `/dist` orderly. If `_test.mjs` is found in test case folder, it will be executed after all entry points are executed.

#### Snapshot testing

Rolldown uses [insta](https://insta.rs/docs/cli/) for rust snapshot testing. You could use

- `cargo insta review` to review the new snapshot one by one.
- `cargo insta accept` to accept all new snapshots at once.

## Node.js Tests

:::tip
Make sure to [build Node.js bindings](./build.md) before running these tests.
:::

### Node.js API tests

Tests located in `packages/rolldown/tests` are used to test Rolldown's Node.js API (i.e. the API of the `rolldown` package published on NPM).

It is our goal to align Rolldown's Node.js API with that of Rollup's as much as possible, and the tests are used to verify API alignment and track the progress. Currently, there are many Rollup options that are not yet supported. If you implemented support for additional options from rollup, please add corresponding test cases for them.

In `/packages/rolldown`:

- `pnpm test` will run rolldown tests.
- `pnpm test:update` will run and update the tests' status.

#### Run tests of the specific file

To run tests of the specific file, you could use

```shell
pnpm test -- test-file-name
```

For example, to run tests in `fixture.test.ts`, you could use `pnpm test -- fixture`.

#### Run the specific test

To run specific test, you could use

```shell
pnpm test -- -t test-name
```

Names of tests in `fixture.test.ts` are defined with their folder names. `tests/fixtures/resolve/alias` will has test name `resolve/alias`.

To run the `tests/fixtures/resolve/alias` test, you could use `pnpm test -- -t "resolve/alias"`.

:::info

- `pnpm test -t aaa bbb` is different from `pnpm test -t "aaa bbb"`. The former will run tests that either contains `aaa` or `bbb`, while the latter will run tests, whose name contain `aaa bbb`.

- For more advanced usage, please refer to https://vitest.dev/guide/filtering.

:::

### Rollup behavior alignment tests

We also aim for behavior alignment with Rollup by running Rollup's own tests against Rolldown.

To achieve this, each test case in `packages/rollup-tests/test` proxies to the corresponding test in the `rollup` git submodule in project root.

The git submodule should have been initialized after running `just init` when setting up the project, but you should also run `just update` to update the submodule before running the Rollup tests.

In `/packages/rollup-tests`:

- `pnpm test` will run rollup tests.
- `pnpm test:update` will run and update the tests' status.
