import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    hookTimeout: process.env.CI ? 1000 * 30 : 1000 * 10,
    // Include playground tests
    include: ['hmr-full-bundle-mode.spec.ts'],
    environment: 'node',
    setupFiles: ['./vitest-setup-playwright.ts'],

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
