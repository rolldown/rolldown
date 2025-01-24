// cSpell:disable
import { defineTest } from 'rolldown-tests'
import { getOutputFileNames } from 'rolldown-tests/utils'
import { expect } from 'vitest'

export default defineTest({
  config: {
    input: ['<main.js'],
    output: {
      sanitizeFileName: (name) => {
        return `sanitized-${name}`
      },
    },
    plugins: [
      {
        name: 'test-plugin',
        async buildStart() {
          this.emitFile({
            type: 'asset',
            name: '<emitted.txt',
            source: 'emitted',
          })
        },
      },
    ],
  },
  afterTest: (output) => {
    expect(getOutputFileNames(output)).toMatchInlineSnapshot(`
      [
        "assets/sanitized-<emitted-umwR9Fta.txt",
        "main.js",
        "sanitized-<dynamic-DyCDX-rK.js",
        "sanitized-<share-DPV8exuF.js",
      ]
    `)
  },
})
