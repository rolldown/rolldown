import type { OutputChunk as RolldownOutputChunk } from 'rolldown'
import { defineTest } from 'rolldown-tests'
import { expect, vi } from 'vitest'

const onLogFn = vi.fn()

export default defineTest({
  config: {
    treeshake: {
      moduleSideEffects: false,
    },
    external: ['test', 'unused-module'],
    onLog(level, log) {
      expect(level).toBe('warn')
      expect(log.code).toBe('UNRESOLVED_IMPORT')
      expect(log.message).toContain(
        "Could not resolve 'unused-external-module' in main.js",
      )
      expect(log.plugin).toBeUndefined()
      onLogFn()
    },
  },
  afterTest: (output) => {
    expect(onLogFn).toHaveBeenCalledTimes(1)

    output.output
      .filter(({ type }) => type === 'chunk')
      .forEach((chunk) => {
        let code = (chunk as RolldownOutputChunk).code
        expect(code.includes(`unused`)).toBe(false)
        expect(code.includes(`unused-module`)).toBe(false)
        expect(code.includes(`b`)).toBe(false)
        expect(code.includes(`b module`)).toBe(false)
      })
  },
})
