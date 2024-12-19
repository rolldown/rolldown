import type { OutputChunk as RolldownOutputChunk } from 'rolldown'
import { defineTest } from '@tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    output: {
      exports: 'named',
      format: 'cjs',
      esModule: true,
    },
  },
  afterTest: (output) => {
    expect(
      output.output
        .filter((output) => output.type === 'chunk' && output.isEntry)
        .every((chunk) =>
          (chunk as RolldownOutputChunk).code.includes(
            "Object.defineProperty(exports, '__esModule', { value: true });",
          ),
        ),
    ).toBe(true)
  },
})
