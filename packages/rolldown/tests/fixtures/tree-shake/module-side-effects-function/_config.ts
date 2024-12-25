import type { OutputChunk as RolldownOutputChunk } from 'rolldown'
import { defineTest } from '@tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    treeshake: {
      moduleSideEffects(id, _) {
        return !id.includes('a.mjs')
      },
    },
  },
  afterTest: (output) => {
    output.output
      .filter(({ type }) => type === 'chunk')
      .forEach((chunk) => {
        let code = (chunk as RolldownOutputChunk).code
        // a.mjs -> module.sideEffects is `false`, `SideEffects::UserDefined(false)` will be used, so the whole module will be deleted
        expect(code.includes(`console.log("a")`)).toBe(false)
        // b.js -> module.sideEffects is `true`, the analyzed side effects is true
        expect(code.includes(`console.log("b")`)).toBe(true)
      })
  },
})
