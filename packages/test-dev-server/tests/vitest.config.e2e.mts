import { resolve } from 'node:path';
import { defineConfig } from 'vitest/config';

// Browser e2e suite (the in-process, auto-discovered harness — see
// meta/design/dev-server-test-harness.md). Discovery lives here and nowhere
// else: any `playground/**/*.spec.ts` is picked up, and the playground name is
// derived from the spec's own path. No central playground registry.
const timeout = process.env.PWDEBUG ? Infinity : process.env.CI ? 50_000 : 30_000;

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
    testTimeout: timeout,
    hookTimeout: timeout,
    // Terse output, matching Vite's playground suite (failures still print full).
    reporters: 'dot',
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
    // No `retry` — vitest defaults to 0.
  },
});
