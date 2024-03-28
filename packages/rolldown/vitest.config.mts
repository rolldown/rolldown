import { defineConfig } from 'vitest/config'
import nodePath from 'node:path'

export default defineConfig({
  test: {
    testTimeout: 20000,
    // Disabled, Because the error printed by rust cannot be seen
    disableConsoleIntercept: true,
  },
  resolve: {
    alias: {
      '@tests': nodePath.resolve(__dirname, 'tests/src'),
    },
  },
  esbuild: {
    target: 'node18',
  },
})
