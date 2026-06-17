# Coding agent guides for `packages/test-dev-server/tests`

## Testing dev-server behavior

The guide for writing these tests — browser playgrounds vs. node fixtures, the
playground layout, the synchronize-don't-`sleep` rule, when to split specs, the
shared-`page` reliability rules, cold-start playgrounds, and the build/run
commands — lives in the **Dev server tests** section of the testing docs. Read
it before adding or changing a test here:

- [`docs/development-guide/testing.md`](../../../docs/development-guide/testing.md#dev-server-tests)
  (published at <https://rolldown.rs/development-guide/testing>)

The deeper "why" behind the harness is in
[`meta/design/dev-server-test-harness.md`](../../../meta/design/dev-server-test-harness.md).
