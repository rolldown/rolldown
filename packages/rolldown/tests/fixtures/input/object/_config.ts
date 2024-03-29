import { defineTest } from '@tests/index'
import path from 'node:path'
import { expect } from 'vitest'
import { getOutputChunkNames } from '@tests/utils'

export default defineTest({
  config: {
    input: {
      main: path.join(__dirname, 'main.js'),
      entry: path.join(__dirname, 'entry.js'),
    },
  },
  afterTest: (output) => {
    expect(getOutputChunkNames(output)).toMatchInlineSnapshot(`
      [
        "entry.js",
        "main.js",
      ]
    `)
  },
})
