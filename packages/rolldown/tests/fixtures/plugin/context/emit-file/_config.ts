import { defineTest } from '@tests'
import { getOutputFileNames } from '@tests/utils'
import { expect } from 'vitest'

let referenceId: string

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin-context',
        async buildStart() {
          referenceId = this.emitFile({
            type: 'asset',
            name: 'emitted.txt',
            source: 'emitted',
          })
        },
        generateBundle() {
          expect(this.getFileName(referenceId)).toMatchInlineSnapshot(
            `"emitted.txt"`,
          )
        },
      },
    ],
  },
  afterTest: (output) => {
    expect(getOutputFileNames(output)).toMatchInlineSnapshot(`
      [
        "emitted.txt",
        "main.js",
      ]
    `)
  },
})
