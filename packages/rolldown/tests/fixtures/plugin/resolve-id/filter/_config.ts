import { defineTest } from '@tests'
import { expect, vi } from 'vitest'

const resolveIdFn = vi.fn()

export default defineTest({
  beforeTest() {},
  skipComposingJsPlugin: true,
  config: {
    input: './main.js',
    plugins: [
      {
        name: 'test-plugin',
        resolveId: {
          filter: {
            id: {
              exclude: [/dir\/a\.js$/],
            },
          },
          handler() {
            resolveIdFn()
            return null
          },
        },
      },
    ],
  },
  afterTest: () => {
    expect(resolveIdFn).toHaveBeenCalledTimes(2)
    resolveIdFn.mockReset()
  },
})
