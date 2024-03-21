import { defineConfig } from 'vitest/config'

export default defineConfig({
  test: {
    include: ['./test/runner.ts', './test/*.test.ts'],
    testTimeout: 20000,
  },
  esbuild: {
    target: 'node18',
  },
})
