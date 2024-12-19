import type { OutputChunk as RolldownOutputChunk } from 'rolldown'
import { defineTest } from '@tests'
import { expect } from 'vitest'

const footerTxt = '// footer test\n'

export default defineTest({
  config: {
    output: {
      footer: footerTxt,
    },
  },
  afterTest: (output) => {
    expect(
      output.output
        .filter(({ type }) => type === 'chunk')
        .every((chunk) =>
          (chunk as RolldownOutputChunk).code.endsWith(footerTxt),
        ),
    ).toBe(true)
  },
})
