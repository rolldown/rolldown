import { defineTest } from '@tests'
import { expect, vi } from 'vitest'

const loadFn = vi.fn()

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        resolveId(id) {
          if (id === 'foo') {
            return {
              id,
            }
          }
        },
        load: {
          filter: {
            id: {
              include: [/^foo$/],
            },
          },
          handler(id) {
            loadFn()
            if (id === 'foo') {
              return {
                code: `console.log('foo')`,
              }
            }
          },
        },
        transform: function (id, code) {
          if (id === 'foo') {
            expect(code).toStrictEqual('')
          }
        },
      },
    ],
  },
  afterTest: () => {
    expect(loadFn).toHaveBeenCalledTimes(1)
  },
})
