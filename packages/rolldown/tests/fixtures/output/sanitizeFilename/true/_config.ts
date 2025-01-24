// cSpell:disable
import { defineTest } from 'rolldown-tests'
import { getOutputFileNames } from 'rolldown-tests/utils'
import { expect } from 'vitest'

export default defineTest({
  config: {
    input: ['<main.js'],
    output: {
      sanitizeFileName: true,
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
        "assets/emitted-umwR9Fta.txt",
        "dynamic-CWS_dRwG.js",
        "main.js",
        "share-ffZ1_1Na.js",
      ]
    `)
  },
})
