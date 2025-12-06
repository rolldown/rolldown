import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    hookTimeout: 1000 * 30,
    // Include Node.js test files (*.spec.ts, not *.browser.test.ts)
    include: ['hmr-full-bundle-mode.spec.ts'],

    // Standard Node.js test environment
    environment: 'node',

    // Setup file that starts dev server and creates Playwright page
    setupFiles: ['./vitest-setup-playwright.ts'],

    // Increase timeout for HMR tests (server startup + file watching)
    testTimeout: 90000,
    expect: {
      poll: {
        timeout: 1000 * 15,
      },
    },
  },
});
