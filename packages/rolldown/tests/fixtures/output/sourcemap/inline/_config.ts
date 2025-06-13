import { expect } from 'vitest'
import { getOutputFileNames } from 'rolldown-tests/utils'
import { defineTest } from 'rolldown-tests'

export default defineTest({
  config: {
    input: ['main.js'],
    output: {
      sourcemap: 'inline',
    },
  },
  afterTest: function (output) {
    expect(getOutputFileNames(output)).toStrictEqual(['main.js'])
    // include data url map comment
    expect(output.output[0].code).contains('//# sourceMappingURL=')
    expect(output.output[0].sourcemapFileName).toBe(null)
  },
})
