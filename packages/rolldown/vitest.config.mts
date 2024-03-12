import { defineConfig } from 'vitest/config'

export default defineConfig({
  test: {
    include: ['./test/runner.ts'],
    testTimeout: 20000,
  },
  esbuild: {
    target: 'node18',
  },
})
