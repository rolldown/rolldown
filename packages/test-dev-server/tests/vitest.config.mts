import nodePath from 'node:path';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    retry: process.env.CI ? 2 : 0,
    testTimeout: 90000, // Increased from 40000 to 90000 for Windows compatibility
    // Disabled, Because the error printed by Rust cannot be seen
    disableConsoleIntercept: true,
    // https://vitest.dev/api/mock.html#mockreset, since we run each test twice, so we need to reset the mockReset for each run
    mockReset: true,
    pool: 'forks',
    poolOptions: {
      forks: {
        singleFork: true,
      },
    },
  },
  resolve: {
    alias: {
      '@tests': nodePath.resolve(__dirname, '../tests/src'),
      '@src': nodePath.resolve(__dirname, '../src'),
    },
  },
  esbuild: {
    target: 'node20',
  },
});
