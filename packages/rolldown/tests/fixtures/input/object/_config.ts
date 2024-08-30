import { defineTest } from '@tests'
import { expect } from 'vitest'
import { getOutputChunkNames } from '@tests/utils'

export default defineTest({
  config: {
    input: {
      main: 'main.js',
      entry: 'entry.js',
    },
  },
  afterTest: (output) => {
    expect(getOutputChunkNames(output)).toStrictEqual(['main.js', 'entry.js'])
  },
})
