import { defineTest } from '@tests'
import { expect, vi } from 'vitest'

const buildStartFn = vi.fn()
const buildStartFn2 = vi.fn()

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        buildStart: function (config) {
          buildStartFn()
          expect(config).toBeTypeOf('object')
        },
      },
      {
        name: 'test-plugin',
        buildStart: {
          handler: buildStartFn2,
        },
      },
    ],
  },
  afterTest: (_output) => {
    expect(buildStartFn).toHaveBeenCalledTimes(1)
    expect(buildStartFn2).toHaveBeenCalledTimes(1)
  },
})
