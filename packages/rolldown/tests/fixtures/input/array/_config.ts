import { defineTest } from '@tests'
import path from 'node:path'
import { expect } from 'vitest'
import { getOutputChunkNames } from '@tests/utils'

export default defineTest({
  config: {
    input: [path.join(__dirname, 'main.js'), path.join(__dirname, 'entry.js')],
  },
  afterTest: function (output) {
    expect(getOutputChunkNames(output)).toMatchInlineSnapshot(`
      [
        "entry.js",
        "main.js",
      ]
    `)
  },
})
