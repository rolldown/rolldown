import type { RollupOptions, RollupOutput } from 'rolldown'
import { expect, vi } from 'vitest'

const loadFn = vi.fn()

const config: RollupOptions = {
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
            map: { mappings: '' },
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
}

export default {
  config,
  afterTest: (output: RollupOutput) => {
    expect(loadFn).toHaveBeenCalledTimes(2)
  },
}
