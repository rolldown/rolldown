import type { OutputChunk as RolldownOutputChunk } from 'rolldown'
import { defineTest } from '@tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    treeshake: {
      moduleSideEffects: false,
    },
    external: ['test', 'unused-module'],
  },
  afterTest: (output) => {
    output.output
      .filter(({ type }) => type === 'chunk')
      .forEach((chunk) => {
        let code = (chunk as RolldownOutputChunk).code
        expect(code.includes(`unused`)).toBe(false)
        expect(code.includes(`unused-module`)).toBe(false)
        expect(code.includes(`b`)).toBe(false)
        expect(code.includes(`b module`)).toBe(false)
      })
  },
})
