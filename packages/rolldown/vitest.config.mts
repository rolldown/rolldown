import { defineConfig } from 'vitest/config'
import nodePath from 'node:path'

export default defineConfig({
  test: {
    testTimeout: 20000,
  },
  resolve: {
    alias: {
      '@tests': nodePath.resolve(__dirname, 'tests'),
    },
  },
  esbuild: {
    target: 'node18',
  },
})
