import { defineTest } from '@tests'
import { expect, vi } from 'vitest'

const fn = vi.fn()

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        outputOptions: function () {
          fn()
        },
      },
    ],
  },
  afterTest: () => {
    expect(fn).toHaveBeenCalledTimes(1)
  },
})
