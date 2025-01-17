import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    output: {
      file: 'dist/out.js',
    },
  },
  afterTest: function (output) {
    expect(output.output[0].fileName).toBe('out.js')
  },
})
