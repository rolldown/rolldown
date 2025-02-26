import { defineTest } from 'rolldown-tests'
import { expect, vi } from 'vitest'

const onLogFn = vi.fn()

export default defineTest({
  config: {
    keepNames: true,
    onLog(level, log) {
      expect(level).toBe('warn')
      expect(log.code).toBe('UNRESOLVED_IMPORT')
      expect(log.message).toContain(
        "Could not resolve 'node:assert' in main.js",
      )
      onLogFn()
    },
  },
  afterTest: async () => {
    expect(onLogFn).toHaveBeenCalledTimes(1)

    await import('./assert.mjs')
  },
})
