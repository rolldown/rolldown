import { defineTest } from '@tests'
import { getOutputChunk } from '@tests/utils'
import { expect, vi } from 'vitest'

const fn = vi.fn()

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        outputOptions: function (options) {
          expect(options.banner).toBeUndefined()
          options.banner = '/* banner */'
          fn()
          return options
        },
      },
    ],
  },
  afterTest: (output) => {
    expect(fn).toHaveBeenCalledTimes(1)
    const chunk = getOutputChunk(output)[0]
    expect(chunk.code.includes('banner')).toBe(true)
  },
})
