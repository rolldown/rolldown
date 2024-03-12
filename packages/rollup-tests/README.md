## Rollup behavior alignment tests

We aim for behavior alignment with Rollup by running Rollup's own tests against Rolldown.

To achieve this, each test case in `packages/rollup-tests/test` proxies to the corresponding test in the `rollup` git submodule in project root.

The git submodule should have been initialized after running `just init` when setting up the project, but you should also run `just update` to update the submodule before running the Rollup tests.

In this directory:

- `pnpm test` will run rollup tests.
- `pnpm test:update` will run and update the tests status.
