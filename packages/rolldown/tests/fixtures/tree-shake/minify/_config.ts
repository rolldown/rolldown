import type { OutputChunk as RolldownOutputChunk } from 'rolldown'
import { defineTest } from 'rolldown-tests'
import { expect, vi } from 'vitest'

const onLogFn = vi.fn()

export default defineTest({
  config: {
    output: {
      minify: true,
    },
    onLog(level, log) {
      expect(level).toBe('warn')
      expect(log.code).toBe('MINIFY_WARNING')
      expect(log.message).toContain(
        'Setting "minify: true" is not recommended for production use.',
      )
      onLogFn()
    },
  },
  afterTest: (output) => {
    expect(onLogFn).toHaveBeenCalledTimes(1)

    output.output
      .filter(({ type }) => type === 'chunk')
      .forEach((chunk) => {
        let code = (chunk as RolldownOutputChunk).code
        // should be mangled, oxc-minify doesn't enable `toplevel` mangle by default
        expect(code.includes(`test`)).toBe(false)
      })
  },
})
