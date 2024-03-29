import type {
  RollupOptions,
  RolldownOutput,
  RolldownOutputChunk,
} from '../../../../src'
import { expect } from 'vitest'

const footerTxt = '// footer test\n'
const footer = () => Promise.resolve().then(() => footerTxt)

const config: RollupOptions = {
  external: [/external/, 'external-a'],
  output: {
    footer,
  },
}

export default {
  config,
  afterTest: (output: RolldownOutput) => {
    expect(
      output.output
        .filter(({ type }) => type === 'chunk')
        .every((chunk) =>
          (chunk as RolldownOutputChunk).code.endsWith(footerTxt),
        ),
    ).toBe(true)
  },
}
