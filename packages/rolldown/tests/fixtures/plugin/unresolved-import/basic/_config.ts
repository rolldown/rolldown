import { defineTest } from '@tests'
import path from 'node:path'

const entry = path.join(__dirname, './main.js')

export default defineTest({
  skip: true,
  skipComposingJsPlugin: true,
  config: {
    input: entry,
    plugins: [
      {
        resolveId(id) {
          if (id === 'test.js') {
            return id
          }
        },
      },
    ],
  },
})
