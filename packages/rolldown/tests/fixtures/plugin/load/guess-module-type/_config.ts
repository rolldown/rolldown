import { defineTest } from '@tests'
import * as fs from 'fs'
import { vi } from 'vitest'

export default defineTest({
  config: {
    input: './main.jsx',
    plugins: [
      {
        name: 'test-plugin',
        load: function (id) {
          let code = fs.readFileSync(id).toString()
          return {
            code,
          }
        },
      },
    ],
  },
  afterTest: (output) => {},
})
