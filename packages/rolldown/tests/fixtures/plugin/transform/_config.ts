import { defineTest } from '@tests'
import { expect, vi } from 'vitest'

const transformFn = vi.fn()

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        transform: function (code, id, meta) {
          transformFn()
          if (id.endsWith('foo.js')) {
            expect(code).toStrictEqual('')
            expect(meta.moduleType).toEqual('js')
            return {
              code: `console.log('transformed')`,
            }
          }
        },
      },
    ],
  },
  afterTest: (output) => {
    expect.assertions(4)
    expect(transformFn).toHaveBeenCalledTimes(2)
    expect(output.output[0].code).contains('transformed')
  },
})
