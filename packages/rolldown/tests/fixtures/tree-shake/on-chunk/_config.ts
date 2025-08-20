import type { OutputChunk as RolldownOutputChunk } from 'rolldown'
import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    treeshake: {
      moduleSideEffects: false,
    },
    output: {
      minify: 'dce-only'
    },
    plugins: [
      {
        name: 'foo',
        renderChunk(code) {
          return code.replace('FOO', 'true');
        },
      }
    ]
  },
  afterTest: (output) => {
    output.output
    .filter(({ type }) => type === 'chunk')
    .forEach((chunk) => {
      let code = (chunk as RolldownOutputChunk).code
      expect(code).not.includes('FOO')
      expect(code).not.includes('true')
      expect(code).includes('console.log("foo")')
    })
  },
})
