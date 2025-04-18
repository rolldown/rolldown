// cSpell:disable
import { defineTest } from 'rolldown-tests'
import { getOutputFileNames } from 'rolldown-tests/utils'
import { expect } from 'vitest'

export default defineTest({
  config: {
    input: ['main.js'],
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
            name: '+emitted.txt',
            source: 'emitted',
          })
          this.emitFile({
            type: 'asset',
            source: 'without-name-and-file-name',
          })
        },
      },
    ],
  },
  afterTest: (output) => {
    expect(getOutputFileNames(output)).toMatchInlineSnapshot(`
      [
        "assets/sanitized-+emitted-C6bBH0W1.txt",
        "assets/sanitized-asset-BIR0xpQL",
        "sanitized-dynamic-C15w8jpw.js",
        "sanitized-main.js",
        "sanitized-share-DR8LfMv5.js",
      ]
    `)
  },
})
