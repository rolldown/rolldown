import type { RollupOptions, RollupOutput } from 'rolldown'
import path from 'node:path'
import { expect } from 'vitest'

const config: RollupOptions = {
  input: [path.join(__dirname, 'main.js')],
  output: {
    sourcemap: true
  }
}

export default {
  config,
  afterTest: function (output: RollupOutput) {
    
  },
}
