# Testing

## Quick Guide

:::tip TLDR
run `just test-update` to run all rust and node.js tests and update snapshots automatically
:::

We have two groups of test suites: one for Rust, and one for Node.js.

:::warning Test principle you should respect

1. When adding new feature with options, always make sure adding related tests in JavaScript side if possible.

Here are some details about how to choose a test technique [details](#how-to-choose-test-technique)
:::

- `just test` for running all tests.
- `just test-update` for running all tests and updating snapshots automatically
- `just test-rust` for running all Rust tests.
- `just test-node` for running all Node.js tests.
- `just test-node-rolldown` for running only Rolldown's Node.js tests.
- `just test-node-rollup` for running only Rollup's tests.

## Concepts

Testing is a crucial part of Rolldown's development process. It helps us ensure the correctness, stability, and performance of the bundler as we add new features and make changes.

Due to the nature of Rolldown being a **bundler**, we prefer integration tests that cover end-to-end scenarios, rather than unit tests that test individual components in isolation. This allows us to verify that the entire bundling process works as expected, from input files to output bundles.

Generally, there are two types of tests we use:

- Data-driven testing: The test runner will look for test cases that follow certain conventions (e.g., folder structure, file naming) and run them automatically. This is the primary way we add new tests.
- Manual testing: For more complex scenarios that cannot be easily expressed with data-driven approaches, we write manual test code that sets up the test environment, runs the bundler with specific options, and verifies the output programmatically.

## Rust

We use Rust's built-in test framework for writing and running tests. Test cases are stored in the `crates/rolldown/tests` folder.

### Data-driven testing

A data-driven test case is a folder that contains a `_config.json` file. The test runner will read the configuration from `_config.json`, bundle the input files, and execute the output files to verify the behavior.

`_config.json` contains the configuration for the test suite. If everything works right, you should be able to have auto-completion while editing `_config.json` due to the [config](https://github.com/rolldown/rolldown/blob/main/.vscode/settings.json#L36-L40).

For all available options, you could refer to

- [Bundler Options](https://github.com/rolldown/rolldown/blob/100c6ee13cef9c50529b8d6425292378ea99eae9/crates/rolldown_common/src/inner_bundler_options/mod.rs#L53)
- [JSON Schema file](https://github.com/rolldown/rolldown/blob/main/crates/rolldown_testing/_config.schema.json)

#### What does data-driven testing do?

- It generates snapshots of the build artifacts, including:
  - Bundled output files
  - Warnings and errors emitted during bundling

- If `_test.mjs` doesn't exist, run the output files in Node.js environment to verify the runtime behavior. You might think of it as running `node --import ./dist/entry1.mjs --import ./dist/entry2.mjs --import ./dist/entry3.mjs --eval ""`.

- Run `_test.mjs` if exists to verify more complex behaviors.

#### Tips

- Snapshots would be updated automatically when you run Rust tests. No extra command is needed.

#### Function-complete data-driven testing

`_config.json` has its limitations, so we also support writing tests with Rust directly. You could refer to

[`crates/rolldown/tests/rolldown/errors/plugin_error`](https://github.com/rolldown/rolldown/blob/86c7aa6557a2bb7eef03133b148b1703f4e21167/crates/rolldown/tests/rolldown/errors/plugin_error)

It basically just replaces the `_config.json` with Rust code that configures the bundler directly. Everything else works the same way as data-driven testing.

#### HMR tests

If a test case folder contains any files named `*.hmr-*.js`, the test will run in HMR enabled mode.

##### HMR edit files

- Files that match the pattern `*.hmr-*.js` are called **HMR edit files**.
- These files represent changes to existing source files.
- The part after `hmr-` indicates the **step number** of the change. For example, `main.hmr-1.js` means a change applied in **step 1**.

##### How the test works

1. All non-HMR files are copied to a temporary directory.
2. An initial build is generated from these files.
3. Then, HMR step 1 begins: files with `.hmr-1.js` are used to overwrite the corresponding files in the temporary directory, and an HMR patch is generated.
4. This process repeats for step 2, 3, and so on. Files like `*.hmr-2.js`, `*.hmr-3.js`, etc., are applied step by step.

:::details Example

If the test folder has these files:

- `main.js`
- `sub.js`
- `main.hmr-1.js`
- `sub.hmr-1.js`
- `sub2.hmr-2.js`

The test will go through these steps:

1. **Initial build**: `main.js`, `sub.js`
2. **Step 1**:
   - `main.js` is replaced with `main.hmr-1.js`
   - `sub.js` is replaced with `sub.hmr-1.js`
3. **Step 2**:
   - `main.js` and `sub.js` remain as in Step 1
   - `sub2.js` is added using the contents of `sub2.hmr-2.js`

:::

### Manual testing

For more complex scenarios that cannot be easily expressed with data-driven approaches, we write manual test code that sets up the test environment, runs the bundler with specific options, and verifies the output programmatically.

Not much to tell here, basically just write normal Rust test code that uses Rolldown to perform bundling and verification.

### test262 Integration Tests

Rolldown integrates the [test262](https://github.com/tc39/test262) test suite to verify ECMAScript specification compliance. Only the test cases under `test/language/module-code` are run because other test cases should be covered on Oxc side.

The git submodule should have been initialized after running `just setup` when setting up the project, but you should also run `just update-submodule` to update the submodule before running the integration tests.

You can run the test262 integration tests with the following command:

```shell
TEST262_FILTER="attribute" cargo test --test integration_test262 -- --no-capture
```

- `TEST262_FILTER` allows you to filter tests by name (e.g., `"attribute"`). If you omit this environment variable, all test cases will be run. Note that the result snapshot will not be updated if the environment variable is set.
- The `--no-capture` option displays all test output.

The test cases that are expected to fail are listed in [`crates/rolldown/tests/test262_failures.json`](https://github.com/rolldown/rolldown/blob/main/crates/rolldown/tests/test262_failures.json).

## Node.js

Rolldown uses [Vitest](https://vitest.dev/) for testing the Node.js side code.

Tests located in `packages/rolldown/tests` are used to test Rolldown's Node.js API (i.e. the API of the `rolldown` package published on NPM).

- `just test-node-rolldown` will run rolldown tests.
- `just test-node-rolldown --update` will run tests and update snapshots.

### Data-driven testing

Data-driven tests are located in `packages/rolldown/tests/fixtures`.

A data-driven test case is a folder that contains a `_config.ts` file. The test runner will read the configuration from `_config.ts`, bundle the input files, and verify the output against expected results.

### Manual testing

Not much to tell here either, basically just write normal JavaScript/TypeScript test code that uses Rolldown to perform bundling and verification.

### Run tests of the specific file

To run tests of the specific file, you could use

```shell
just test-node-rolldown test-file-name
```

For example, to run tests in `fixture.test.ts`, you could use `just test-node-rolldown fixture`.

### Tips

#### Run the specific test

To run specific test, you could use

```shell
just test-node-rolldown -t test-name
```

Names of tests in `fixture.test.ts` are defined with their folder names. `tests/fixtures/resolve/alias` will has test name `resolve/alias`.

To run the `tests/fixtures/resolve/alias` test, you could use `just test-node-rolldown -t resolve/alias`.

:::info

- `just test-node-rolldown -t aaa bbb` is different from `just test-node-rolldown -t "aaa bbb"`. The former will run tests that either contains `aaa` or `bbb`, while the latter will run tests, whose name contain `aaa bbb`.

- For more advanced usage, please refer to https://vitest.dev/guide/filtering.

:::

## Rollup behavior alignment tests

We also aim for behavior alignment with Rollup by running Rollup's own tests against Rolldown.

To achieve this, each test case in `packages/rollup-tests/test` proxies to the corresponding test in the `rollup` git submodule in project root.

The git submodule should have been initialized after running `just setup` when setting up the project, but you should also run `just update-submodule` to update the submodule before running the Rollup tests.

In `/packages/rollup-tests`:

- `just test-node-rollup` will run rollup tests.
- `just test-node-rollup --update` will run and update the tests' status.

## How to choose test technique

Our Rust test infra is powerful enough to cover most of the case of JavaScript (plugin, passing function inside config).
But since JavaScript side user is still our first class user, try to put tests in JavaScript side if possible.
Here are some experience about what test technique you should use.
:::tip TLDR
Add test in JavaScript side if you don't want to wasting time on deciding which way to use.
:::

#### Prefer Rust

1. Test warning or error emitted by rolldown core.
   - [error](https://github.com/rolldown/rolldown/blob/568197a06444809bf44642d88509313ee2735594/crates/rolldown/tests/rolldown/errors/assign_to_import/artifacts.snap?plain=1#L2-L54)
   - [warning](https://github.com/rolldown/rolldown/blob/568197a06444809bf44642d88509313ee2735594/crates/rolldown/tests/rolldown/warnings/eval/artifacts.snap?plain=1#L1-L28)
2. Matrix testing, assume you want to test a suite different [format](https://github.com/rolldown/rolldown/blob/568197a06444809bf44642d88509313ee2735594/crates/rolldown/tests/rolldown/topics/bundler_esm_cjs_tests/4/_config.json?plain=1#L1-L21), with `configVariants` you could do that with only one test.
3. Tests related to linking algorithm(tree shaking, chunk splitting) Those may require a lot of debugging, add test on Rust side could reduce the time of coding-debug-coding work loop.

#### Prefer JavaScript

Any category not mentioned above should put in JavaScript side.
