import { defineTest } from '@tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    output: {
      file: 'out.js',
    },
  },
  afterTest: function (output) {
    expect(output.output[0].fileName).toMatchInlineSnapshot(`"out.js"`)
  },
})
