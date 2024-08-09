import { defineTest } from '@tests'

export default defineTest({
  config: {
    output: {
      exports: 'named',
      format: 'iife',
      name: 'module',
      extend: true,
    },
  },
})
