import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'
import { getOutputChunkNames } from 'rolldown-tests/utils'

export default defineTest({
  config: {
    input: {
      main: 'main.js',
      entry: 'entry.js',
    },
  },
  afterTest: (output) => {
    expect(getOutputChunkNames(output)).toStrictEqual(['entry.js', 'main.js'])
  },
})
