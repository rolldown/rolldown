import { defineTest } from 'rolldown-tests'
import { getOutputFileNames } from 'rolldown-tests/utils'
import { expect } from 'vitest'

export default defineTest({
  config: {
    input: ['main.js'],
    output: {
      sanitizeFileName: true,
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
        },
      },
    ],
  },
  afterTest: (output) => {
    expect(getOutputFileNames(output)).toMatchInlineSnapshot(`
      [
        "assets/_emitted-C6bBH0W1.txt",
        "main.js",
      ]
    `)
  },
})
