import { defineTest } from '@tests'
import { expect, vi } from 'vitest'
import path from 'node:path'

const fn = vi.fn()

export default defineTest({
  config: {
    input: [],
    plugins: [
      {
        name: 'test-plugin',
        options: function (opts) {
          opts.input = [path.join(__dirname, 'main.js')]
          fn()
        },
      },
    ],
  },
  afterTest: () => {
    expect(fn).toHaveBeenCalledTimes(1)
  },
})
