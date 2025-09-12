## Rollup behavior alignment tests

We aim for behavior alignment with Rollup by running Rollup's own tests against Rolldown.

To achieve this, each test case in `packages/rollup-tests/test` proxies to the corresponding test in the `rollup` git submodule in project root.

The git submodule should have been initialized after running `just setup` when setting up the project, but you should also run `just update-submodule` to update the submodule before running the Rollup tests.


- `just test-node-rollup` will run rollup tests.
- `just test-node-rollup --update` will run and update the tests status.
