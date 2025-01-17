import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'
import { getOutputChunkNames } from 'rolldown-tests/utils'

export default defineTest({
  config: {
    input: 'main.js',
  },
  afterTest: (output) => {
    expect(getOutputChunkNames(output)).toStrictEqual(['main.js'])
  },
})
