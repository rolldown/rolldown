// cSpell:disable
import { expect } from 'vitest'
import { getOutputFileNames } from '@tests/utils'
import { defineTest } from '@tests'

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
