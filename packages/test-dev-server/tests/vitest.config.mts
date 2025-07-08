import nodePath from 'node:path';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    testTimeout: 40000,
    // Disabled, Because the error printed by rust cannot be seen
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
    target: 'node18',
  },
});
