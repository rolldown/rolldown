import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'
import {
  getLocation,
  getOutputAsset,
  getOutputChunk,
} from 'rolldown-tests/utils'
import { SourceMapConsumer } from 'source-map'

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        transform(code) {
          return {
            code,
            map: {
              version: 3,
              names: [],
              sources: ['test.tsx'],
              // intentionally passing null
              sourceRoot: null as unknown as string | undefined,
              sourcesContent: ["export const foo = 'foo';\n"],
              mappings: 'AAAA,OAAO,MAAM,MAAM',
            },
          }
        },
      },
    ],
    output: {
      sourcemap: true,
    },
  },
  afterTest: async (output) => {
    const code = getOutputChunk(output)[0].code
    const map = getOutputAsset(output)[0].source as string
    const smc = await new SourceMapConsumer(JSON.parse(map))

    const generatedLoc = getLocation(code, code.indexOf(`"foo"`))
    const originalLoc = smc.originalPositionFor(generatedLoc)

    expect(originalLoc.line).toBe(1)
    expect(originalLoc.column).toBe(19)
  },
})
