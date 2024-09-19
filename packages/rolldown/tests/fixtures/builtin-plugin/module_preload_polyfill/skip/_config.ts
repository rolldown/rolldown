import { modulePreloadPolyfillPlugin } from 'rolldown/experimental'
import { RolldownOutput } from 'rolldown'
import { defineTest } from '@tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    plugins: [
      modulePreloadPolyfillPlugin({
        skip: true,
      }),
    ],
  },

  afterTest(output: RolldownOutput) {
    expect(output.output[0].code.length).not.toBe(0)
    expect(output.output[0].code).contain('this should be kept')
  },
})
