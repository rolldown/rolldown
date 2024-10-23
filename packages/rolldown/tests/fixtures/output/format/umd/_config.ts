import { defineTest } from '@tests'
import { expect } from 'vitest'

let isComposingJs = false
export default defineTest({
  beforeTest(testKind) {
    isComposingJs = testKind === 'compose-js-plugin'
  },
  config: {
    external: /node:path/,
    output: {
      exports: 'named',
      name: 'module',
      format: 'umd',
      globals: {
        'node:path': 'path',
      },
    },
  },
  afterTest: (output) => {
    expect(output.output[0].code).toMatchSnapshot(
      `isComposingJs: ${isComposingJs}`,
    )
  },
})
