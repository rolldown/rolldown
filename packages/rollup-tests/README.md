The folder `rollup-tests/test` is copied from https://github.com/rollup/rollup/tree/master/test

# Keep the tests up to date

1. Copy https://github.com/rollup/rollup/tree/master/test to replace folder `rollup-tests/test`.

2. Run `yarn test` to check if all tests pass.

# Scripts

## yarn test

Run all tests but skip the ones in `src/failed-tests.json`.

## yarn test:update

Only run tests in `src/failed-tests.json`. If the test passes, the update test case will be removed.
