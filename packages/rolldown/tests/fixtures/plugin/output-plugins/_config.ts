import { defineTest } from 'rolldown-tests'
import { getOutputChunk } from 'rolldown-tests/utils'
import { expect, vi } from 'vitest'

const fn = vi.fn()
const renderStartFn = vi.fn()
const onLogFn = vi.fn()

export default defineTest({
  config: {
    output: {
      plugins: [
        {
          name: 'test-plugin',
          // @ts-expect-error test warning
          buildStart: () => {},
          outputOptions: function (options) {
            expect(options.banner).toBeUndefined()
            options.banner = '/* banner */'
            fn()
            return options
          },
          renderStart: renderStartFn,
        },
      ],
    },
    onLog(level, log) {
      expect(level).toBe('warn')
      expect(log.code).toBe('INPUT_HOOK_IN_OUTPUT_PLUGIN')
      onLogFn()
    },
  },
  afterTest: (output) => {
    expect(onLogFn).toHaveBeenCalledTimes(1)
    expect(renderStartFn).toHaveBeenCalledTimes(1)
    expect(fn).toHaveBeenCalledTimes(1)
    const chunk = getOutputChunk(output)[0]
    expect(chunk.code.includes('banner')).toBe(true)
  },
})
