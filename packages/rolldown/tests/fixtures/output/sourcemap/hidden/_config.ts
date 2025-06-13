import { expect } from 'vitest'
import { getOutputFileNames } from 'rolldown-tests/utils'
import { defineTest } from 'rolldown-tests'

export default defineTest({
  config: {
    input: ['main.js'],
    output: {
      sourcemap: 'hidden',
    },
  },
  afterTest: function (output) {
    expect(getOutputFileNames(output)).toStrictEqual(['main.js', 'main.js.map'])
    // not include map comment
    expect(output.output[0].code).not.contains('//# sourceMappingURL=')
    expect(output.output[0].sourcemapFileName).toBe('main.js.map')
    expect(output.output[0].map).toBeDefined()
  },
})
