import type { OutputChunk as RolldownOutputChunk } from 'rolldown'
import { defineTest } from '@tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    treeshake: {
      moduleSideEffects: [
        {
          test: /a\.mjs/,
          sideEffects: true,
        },
        {
          test: /\.js/,
          sideEffects: false,
        },
        {
          test: /b\.js/,
          sideEffects: true,
        },
      ],
    },
  },
  afterTest: (output) => {
    let chunk = output.output.filter(({ type }) => type === 'chunk')[0]
    let code = (chunk as RolldownOutputChunk).code
    // a.mjs -> module.sideEffects is `true`
    expect(code.includes(`console.log("a")`)).toBe(true)
    // b.js -> module.sideEffects is `false`, `/\.js/`  has higher priority than `/b\.js/`
    expect(code.includes(`console.log("b")`)).toBe(false)
  },
})
