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
            code: code + '\nconsole.log("added")',
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

    const generatedLoc = getLocation(code, code.indexOf(`"main"`))
    const originalLoc = smc.originalPositionFor(generatedLoc)
    expect(originalLoc.line).toBe(1)
    expect(originalLoc.column).toBe(12)
    expect(smc.sourceContentFor(originalLoc.source!)).toBe(
      "console.log('main')",
    )

    const generatedLoc2 = getLocation(code, code.indexOf(`"added"`))
    const originalLoc2 = smc.originalPositionFor(generatedLoc2)
    expect(originalLoc2.line).toBe(null)
    expect(originalLoc2.column).toBe(null)
    expect(originalLoc2.source).toBe(null)
  },
})
