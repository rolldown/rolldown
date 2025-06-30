import path from 'node:path'

import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'
import type { InternalModuleFormat, NormalizedOutputOptions } from 'rolldown'

const entry = path.join(__dirname, './main.js')

let generateBundleOutputOptions: Partial<Record<InternalModuleFormat, NormalizedOutputOptions>> = {}

export default defineTest({
  config: {
    input: entry,
    plugins: [
      {
        name: 'test-plugin',
        writeBundle: async (options) => {
          expect(generateBundleOutputOptions[options.format]).not.toBeUndefined()
          // ensure same reference
          expect(options).toBe(generateBundleOutputOptions[options.format])
        },
      },
      {
        name: 'test-plugin-save-generate-bundle-output-options',
        generateBundle: async (options) => {
          generateBundleOutputOptions[options.format] = options
        },
      }
    ],
    output: [
      { format: 'es' },
      { format: 'cjs' }
    ]
  },
})
