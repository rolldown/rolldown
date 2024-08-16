import { defineTest } from '@tests'
import fs from 'node:fs'

export default defineTest({
  config: {
    input: './main.typescript',
    plugins: [
      {
        name: 'rewrite-module-type',
        load: function (id) {
          debugger
          return {
            code: fs.readFileSync(id, 'utf-8'),
            moduleType: 'ts',
          }
        },
      },
    ],
  },
  afterTest: async (output) => {
    await import('./assert.mjs')
  },
})
