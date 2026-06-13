import { resolve } from 'node:path';
import { defineConfig } from 'vitest/config';

// Browser e2e suite (the in-process, auto-discovered harness — see
// meta/design/dev-server-test-harness.md). Discovery lives here and nowhere
// else: any `playground/**/*.spec.ts` is picked up, and the playground name is
// derived from the spec's own path. No central playground registry.

// DEBUG (CI flake #9727): allow overriding timeout/retry from the CI env so a
// repro run can finish inside the 15-min job budget without a code edit.
const timeout = process.env.RD_TEST_TIMEOUT
  ? Number(process.env.RD_TEST_TIMEOUT)
  : process.env.CI
    ? 50_000
    : 30_000;

export default defineConfig({
  resolve: {
    alias: {
      '~utils': resolve(import.meta.dirname, './playground/test-utils'),
    },
  },
  test: {
    include: ['./playground/**/*.spec.[tj]s'],
    setupFiles: ['./playground/vitest-setup.ts'],
    globalSetup: ['./playground/vitest-global-setup.ts'],
    // Each spec file owns an in-process dev engine (rust/napi threads); a fork
    // per file keeps them isolated. Sequential for now — the parallelism trial
    // is Phase 4 (meta/design/dev-server-test-harness.md, Unresolved Q1).
    pool: 'forks',
    fileParallelism: false,
    testTimeout: timeout,
    hookTimeout: timeout,
    // Test knobs that the subprocess model passed via child env now live on the
    // harness process itself (it runs the dev engine in-process).
    env: {
      RUST_BACKTRACE: process.env.RUST_BACKTRACE ?? 'FULL',
      RD_LOG: process.env.RD_LOG ?? 'hmr=debug',
    },
    expect: {
      poll: {
        timeout: process.env.CI ? 1000 * 10 : 1000 * 3,
      },
    },
    // Retained through the migration; removal is Phase 4 once the determinism
    // crutches (port races, orphaned subprocesses) are gone.
    // DEBUG (CI flake #9727): `RD_TEST_RETRY=0` makes a repro fail fast so the
    // first-attempt cause shows up without 3 retries blowing the job budget.
    retry: process.env.RD_TEST_RETRY ? Number(process.env.RD_TEST_RETRY) : process.env.CI ? 3 : 1,
  },
});
