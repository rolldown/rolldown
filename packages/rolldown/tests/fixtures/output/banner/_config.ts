import type { RolldownOutputChunk } from 'rolldown'
import { defineTest } from '@tests'
import { expect } from 'vitest'

const bannerTxt = '// banner test\n'
const banner = () => Promise.resolve().then(() => bannerTxt)

export default defineTest({
  config: {
    external: [/external/, 'external-a'],
    output: {
      banner,
    },
  },
  afterTest: (output) => {
    expect(
      output.output
        .filter(({ type }) => type === 'chunk')
        .every((chunk) =>
          (chunk as RolldownOutputChunk).code.startsWith(bannerTxt),
        ),
    ).toBe(true)
  },
})
