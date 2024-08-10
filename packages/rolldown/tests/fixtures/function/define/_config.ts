import { defineTest } from '@tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    define: {
      'process.env.NODE_ENV': '"production"',
    },
  },
  afterTest: function (output) {
    expect(output.output[0].code).not.includes('process.env.NODE_ENV')
    expect(output.output[0].code).includes('production')
  },
})
