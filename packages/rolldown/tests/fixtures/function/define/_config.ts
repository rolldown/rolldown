import { defineTest } from 'rolldown-tests'
import { expect, vi } from 'vitest'
import nodePath from 'node:path'

const onLogFn = vi.fn()

export default defineTest({
  config: {
    define: {
      'process.env.NODE_ENV': '"production"',
    },
    onLog(level, log) {
      expect(level).toBe('warn')
      expect(log.code).toBe('UNRESOLVED_IMPORT')
      expect(log.message).toContain(
        "Could not resolve 'node:assert' in main.js",
      )
      onLogFn()
    },
  },
  async afterTest(output) {
    expect(onLogFn).toHaveBeenCalledTimes(1)

    await expect(output.output[0].code).toMatchFileSnapshot(
      nodePath.join(import.meta.dirname, 'output.snap'),
    )
  },
})
