import { defineTest } from '@tests'

export default defineTest({
  config: {
    external: /node:path/,
    output: {
      exports: 'named',
      format: 'iife',
    },
  },
})
