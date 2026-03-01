import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    hookTimeout: process.env.CI ? 1000 * 30 : 1000 * 10,
    // Include playground tests
    include: ['hmr-full-bundle-mode.spec.ts', 'lazy-compilation.spec.ts'],
    environment: 'node',
    // Use globalSetup for one-time initialization (server start, tmp dirs).
    // This runs ONCE before all test files, unlike setupFiles which runs per-file.
    // This prevents EBUSY errors on Windows when the second test file tries to
    // recreate tmp directories while the first file's servers are still running.
    globalSetup: ['./vitest-global-setup-browser.ts'],
    // setupFiles still runs per-file for test hooks (beforeEach, etc.)
    setupFiles: ['./vitest-setup-browser.ts'],
    // Disable file parallelism to prevent race conditions in shared setup
    fileParallelism: false,

    // Increase timeout for HMR tests (server startup + file watching)
    testTimeout: 90000,
    expect: {
      poll: {
        timeout: process.env.CI ? 1000 * 10 : 1000 * 3,
      },
    },
    // Enable retry for flaky tests in CI
    retry: process.env.CI ? 3 : 1,
  },
});
