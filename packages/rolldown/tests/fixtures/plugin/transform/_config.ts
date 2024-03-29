import { defineTest } from '@tests'
import { expect, vi } from 'vitest'

const transformFn = vi.fn()

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        transform: function (code, id) {
          transformFn()
          if (id.endsWith('foo.js')) {
            expect(code).toStrictEqual('')
            return {
              code: `console.log('transformed')`,
            }
          }
        },
      },
    ],
  },
  afterTest: (output) => {
    expect.assertions(3)
    expect(transformFn).toHaveBeenCalledTimes(2)
    expect(output.output[0].code).contains('transformed')
  },
})
