import { defineTest } from '@tests'

export default defineTest({
  config: {
    external: /node:path/,
    output: {
      format: 'iife',
      globals: {
        'node:path': 'path',
      }
    }
  },
})
