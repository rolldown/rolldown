import { defineTest } from '@tests'
import { expect, vi } from 'vitest'

const loadFn = vi.fn()

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        resolveId: function (id, importer, options) {
          if (id === 'foo') {
            return {
              id,
            }
          }
        },
        load: function (id) {
          loadFn()
          if (id === 'foo') {
            return {
              code: `console.log('foo')`,
            }
          }
        },
        transform: function (id, code) {
          if (id === 'foo') {
            expect(code).toStrictEqual('')
          }
        },
      },
    ],
  },
  afterTest: (output) => {
    expect(loadFn).toHaveBeenCalledTimes(2)
  },
})
