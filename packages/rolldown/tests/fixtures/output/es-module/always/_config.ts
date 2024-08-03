import type { RolldownOutputChunk } from 'rolldown'
import { defineTest } from '@tests'
import { expect } from 'vitest'

const footerTxt = '// footer test\n'

export default defineTest({
  config: {
    output: {
      exports: 'named',
      format: 'cjs',
      esModule: 'always',
    },
  },
  afterTest: (output) => {
    expect(
      output.output
        .filter(({ type }) => type === 'chunk')
        .every((chunk) =>
          (chunk as RolldownOutputChunk).code.includes(
            "Object.defineProperty(exports, '__esModule', { value: true });",
          ),
        ),
    ).toBe(true)
  },
})
