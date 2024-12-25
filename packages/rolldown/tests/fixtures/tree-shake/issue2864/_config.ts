import { defineTest } from '@tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    input: {
      main: './entry.js',
    },
    plugins: [
      {
        name: 'loader',
        transform(_code, id) {
          return {
            moduleSideEffects: id.endsWith('sideeffect.js'),
          }
        },
      },
    ],
  },
  afterTest: (output) => {
    expect(output.output[0].code).toContain('globalThis.aa = true')
  },
})
