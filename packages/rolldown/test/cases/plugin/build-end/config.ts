import type { RollupOptions, RollupOutput } from 'rolldown'
import { expect, vi } from 'vitest'

const buildEndFn = vi.fn()

const config: RollupOptions = {
  plugins: [
    {
      name: 'test-plugin',
      buildEnd: function (err) {
        buildEndFn()
        expect(err).toBeUndefined()
      },
    },
  ],
}

export default {
  config,
  afterTest: (output: RollupOutput) => {
    expect(buildEndFn).toHaveBeenCalledTimes(1)
  },
}
