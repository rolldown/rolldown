import type {
  RollupOptions,
  RolldownOutput,
  RolldownOutputChunk,
} from '../../../../src'
import { expect } from 'vitest'

const bannerTxt = '// banner test\n'
const banner = () => Promise.resolve().then(() => bannerTxt)

const config: RollupOptions = {
  external: [/external/, 'external-a'],
  output: {
    banner,
  },
}

export default {
  config,
  afterTest: (output: RolldownOutput) => {
    expect(
      output.output
        .filter(({ type }) => type === 'chunk')
        .every((chunk) =>
          (chunk as RolldownOutputChunk).code.startsWith(bannerTxt),
        ),
    ).toBe(true)
  },
}
