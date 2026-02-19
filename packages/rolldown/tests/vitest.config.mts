import nodePath from 'node:path';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    testTimeout: 20000,
    disableConsoleIntercept: true,
    pool: 'forks',
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
