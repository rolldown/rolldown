// cSpell:disable
import { expect } from 'vitest'
import { getOutputFileNames } from 'rolldown-tests/utils'
import { defineTest } from 'rolldown-tests'

export default defineTest({
  config: {
    input: ['main.js'],
    output: {
      sourcemap: false,
    },
  },
  afterTest: function (output) {
    expect(getOutputFileNames(output)).toStrictEqual(['main.js'])
  },
})
