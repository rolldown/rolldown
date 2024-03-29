import { defineTest } from '@tests/index'
import path from 'node:path'
import { expect } from 'vitest'
import { getOutputChunkNames } from '@tests/utils'

export default defineTest({
  config: {
    input: path.join(__dirname, 'main.js'),
  },
  afterTest: (output) => {
    expect(getOutputChunkNames(output)).toStrictEqual(['main.js'])
  },
})
