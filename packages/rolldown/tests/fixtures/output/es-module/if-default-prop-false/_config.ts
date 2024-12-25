import type { OutputChunk as RolldownOutputChunk } from 'rolldown'
import { defineTest } from '@tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    output: {
      exports: 'named',
      format: 'iife',
      esModule: 'if-default-prop',
    },
  },
  afterTest: (output) => {
    expect(
      output.output
        .filter(({ type }) => type === 'chunk')
        .every(
          (chunk) =>
            !(chunk as RolldownOutputChunk).code.includes(
              "Object.defineProperty(exports, '__esModule', { value: true });",
            ),
        ),
    ).toBe(true)
  },
})
