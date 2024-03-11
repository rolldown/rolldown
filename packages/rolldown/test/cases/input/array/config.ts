import type { RollupOptions, RollupOutput } from 'rolldown'
import path from 'node:path'
import { expect } from 'vitest'
import { getOutputChunkNames } from '../../../util'

const config: RollupOptions = {
  input: [path.join(__dirname, 'main.js'), path.join(__dirname, 'entry.js')],
}

export default {
  config,
  afterTest: function (output: RollupOutput) {
    expect(getOutputChunkNames(output)).toMatchInlineSnapshot(`
      [
        "entry.js",
        "main.js",
      ]
    `)
  },
}
