import { defineTest } from '@tests'
import { expect, vi } from 'vitest'

const buildEndFn = vi.fn()
const buildEndFn2 = vi.fn()

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        buildEnd: function (err) {
          buildEndFn()
          expect(err).toBeNull()
        },
      },
      {
        name: 'test-plugin-2',
        buildEnd: buildEndFn2,
      },
    ],
  },
  afterTest: () => {
    expect(buildEndFn).toHaveBeenCalledTimes(1)
    expect(buildEndFn2).toHaveBeenCalledTimes(1)
  },
})
