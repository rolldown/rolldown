import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    hookTimeout: 1000 * 30,
    // Include playground tests
    include: ['hmr-full-bundle-mode.spec.ts'],
    environment: 'node',
    setupFiles: ['./vitest-setup-playwright.ts'],
    retry: process.env.CI ? 3 : 0,

    // Increase timeout for HMR tests (server startup + file watching)
    testTimeout: 90000,
    expect: {
      poll: {
        timeout: 1000 * 10,
      },
    },
  },
});
