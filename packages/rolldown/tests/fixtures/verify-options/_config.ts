import { expect } from 'vitest'
import { defineTest } from '@tests'

export default defineTest({
  config: {
    output: {
      target: 'es2015',
    },
    plugins: [
      {
        name: 'test-plugin',
        outputOptions: function (options) {
          expect(options.target).toBe('es2015')
        },
      },
    ],
  },
})
