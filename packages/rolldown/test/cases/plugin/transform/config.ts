import type { RollupOptions, RollupOutput } from 'rolldown'
import { expect, vi } from 'vitest'

const transformFn = vi.fn()

const config: RollupOptions = {
  plugins: [
    {
      name: 'test-plugin',
      transform: function (code, id) {
        transformFn()
        if (id.endsWith('foo.js')) {
          expect(code).toStrictEqual('')
          return {
            code: `console.log('transformed')`,
            map: { mappings: '' },
          }
        }
      },
    },
  ],
}

export default {
  config,
  afterTest: (output: RollupOutput) => {
    expect.assertions(3)
    expect(transformFn).toHaveBeenCalledTimes(2)
    expect(output.output[0].code).contains('transformed')
  },
}
