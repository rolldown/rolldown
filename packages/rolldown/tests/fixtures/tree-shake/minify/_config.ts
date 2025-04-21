import type { OutputChunk as RolldownOutputChunk } from 'rolldown'
import { defineTest } from 'rolldown-tests'
import { expect, vi } from 'vitest'

export default defineTest({
  config: {
    output: {
      minify: true,
    }
  },
  afterTest: (output) => {
    output.output
      .filter(({ type }) => type === 'chunk')
      .forEach((chunk) => {
        let code = (chunk as RolldownOutputChunk).code
        // should be mangled, oxc-minify doesn't enable `toplevel` mangle by default
        expect(code.includes(`test`)).toBe(false)
      })
  },
})
