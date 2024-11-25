import { defineTest } from '@tests'
import { getOutputChunk } from '@tests/utils'
import { expect, vi } from 'vitest'

const fn = vi.fn()
const renderStartFn = vi.fn()

export default defineTest({
  skipComposingJsPlugin: true, // Here mutate the test config at non-skipComposingJsPlugin test will be next skipComposingJsPlugin test failed.
  config: {
    output: {
      plugins: [
        {
          name: 'test-plugin',
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
  },
  afterTest: (output) => {
    expect(renderStartFn).toHaveBeenCalledTimes(1)
    expect(fn).toHaveBeenCalledTimes(1)
    const chunk = getOutputChunk(output)[0]
    expect(chunk.code.includes('banner')).toBe(true)
  },
})
