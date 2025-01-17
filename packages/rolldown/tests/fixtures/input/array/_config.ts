import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'
import { getOutputChunkNames } from 'rolldown-tests/utils'

export default defineTest({
  config: {
    input: ['main.js', 'entry.js'],
  },
  afterTest: function (output) {
    expect(getOutputChunkNames(output)).toStrictEqual(['entry.js', 'main.js'])
  },
})
