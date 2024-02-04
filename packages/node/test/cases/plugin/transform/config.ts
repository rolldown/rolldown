import type { RollupOptions, RollupOutput } from '@rolldown/node'
import { expect, vi } from 'vitest'

const transformFn = vi.fn()

const config: RollupOptions = {
  plugins: [
    {
      name: 'test-plugin',
      transform: function (id, code) {
        transformFn()
        if (id.endsWith('foo.js')) {
          expect(code).toStrictEqual('')
          return {
            code: `console.log('foo')`,
            map: { mappings: "" }
          }
        }
      },
    },
  ],
}

export default {
  config,
  afterTest: (output: RollupOutput) => {
    expect(transformFn).toHaveBeenCalledTimes(2)
  },
}
