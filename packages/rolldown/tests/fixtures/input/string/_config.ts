import type { RollupOptions, RollupOutput } from 'rolldown'
import path from 'node:path'
import { expect } from 'vitest'
import { getOutputChunkNames } from '../../../util'

const config: RollupOptions = {
  input: path.join(__dirname, 'main.js'),
}

export default {
  config,
  afterTest: (output: RollupOutput) => {
    expect(getOutputChunkNames(output)).toStrictEqual(['main.js'])
  },
}
