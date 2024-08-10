import { defineTest } from '@tests'
import { expect, vi } from 'vitest'

const transformFn = vi.fn()

export default defineTest({
  config: {
    input: './main.typescript',
    plugins: [
      {
        name: 'rewrite-module-type',
        transform: function (code, id, meta) {
          return {
            moduleType: 'ts',
          }
        },
      },
    ],
  },
  afterTest: async (output) => {
    await import('./assert.mjs')
  },
})
