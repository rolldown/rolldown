# Coding agent guides for `packages/test-dev-server/tests`

## Testing dev-server behavior

The guide for writing these tests — browser playgrounds vs. node fixtures, the
playground layout, the required Vite submodule setup, the assertion signals,
the synchronize-don't-`sleep` rule, when to split specs, the shared-`page`
reliability rules, cold-start playgrounds, and the build/run commands — lives
in the **Dev server tests** section of the testing docs. Read it before adding
or changing a test here:

- [`docs/development-guide/testing.md`](../../../docs/development-guide/testing.md#dev-server-tests)
  (published at <https://rolldown.rs/development-guide/testing>)

The harness architecture (the Vite full-bundle-mode backend, the node
transport, the submodule policy) is in
[`internal-docs/dev-server-test-harness/implementation.md`](../../../internal-docs/dev-server-test-harness/implementation.md).
