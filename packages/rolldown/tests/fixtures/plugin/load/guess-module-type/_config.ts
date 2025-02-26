import { defineTest } from 'rolldown-tests'
import * as fs from 'fs'

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
    external: ['react/jsx-runtime'],
  },
})
