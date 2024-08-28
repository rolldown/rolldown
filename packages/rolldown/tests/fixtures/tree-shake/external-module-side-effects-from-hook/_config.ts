import { defineTest } from '@tests'
import { expect } from 'vitest'
import path from 'node:path'

const entry = path.join(__dirname, './main.js')

export default defineTest({
  config: {
    input: entry,
    plugins: [
      {
        name: 'test-plugin',
        resolveId: function (id, importer, options) {
          if (id === 'external') {
            return {
              id,
              external: true,
              moduleSideEffects: false,
            }
          }
        },
      },
    ],
  },
  afterTest: (output) => {
    expect(output.output[0].code.includes('external')).toBe(false)
  },
})
