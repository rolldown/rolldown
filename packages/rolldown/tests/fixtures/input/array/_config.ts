import { defineTest } from '@tests'
import { expect } from 'vitest'
import { getOutputChunkNames } from '@tests/utils'

export default defineTest({
  config: {
    input: ['main.js', 'entry.js'],
  },
  afterTest: function (output) {
    expect(getOutputChunkNames(output)).toStrictEqual(['main.js', 'entry.js'])
  },
})
