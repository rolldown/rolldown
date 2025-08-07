import type { OutputChunk as RolldownOutputChunk } from 'rolldown'
import { defineTest } from 'rolldown-tests'
import { expect, vi } from 'vitest'

const onLogFn = vi.fn()

export default defineTest({
  config: {
    output: {
      exports: 'named',
      format: 'iife',
      esModule: 'if-default-prop',
    },
    onLog(level, log) {
      expect(level).toBe('warn')
      expect(log.code).toBe('MISSING_NAME_OPTION_FOR_IIFE_EXPORT')
      expect(log.plugin).toBeUndefined()
      onLogFn()
    },
  },
  afterTest: (output) => {
    expect(onLogFn).toHaveBeenCalledTimes(1)

    expect(
      output.output
        .filter(({ type }) => type === 'chunk')
        .every((chunk) =>
          (chunk as RolldownOutputChunk).code.includes(
            "Object.defineProperty(exports, '__esModule', { value: true });",
          ),
        ),
    ).toBe(true)
  },
})
