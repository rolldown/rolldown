import type { RollupOptions, RollupOutput } from 'rolldown'
import { expect, vi } from 'vitest'

const buildStartFn = vi.fn()

const config: RollupOptions = {
  plugins: [
    {
      name: 'test-plugin',
      buildStart: function (config) {
        buildStartFn()
        expect(config).toBeTypeOf('object')
      },
    },
  ],
}

export default {
  config,
  afterTest: (output: RollupOutput) => {
    expect(buildStartFn).toHaveBeenCalledTimes(1)
  },
}
