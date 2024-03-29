import type { RolldownOutputChunk } from 'rolldown'
import { defineTest } from '@tests'
import { expect } from 'vitest'

const footerTxt = '// footer test\n'
const footer = () => Promise.resolve().then(() => footerTxt)

export default defineTest({
  config: {
    external: [/external/, 'external-a'],
    output: {
      footer,
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
