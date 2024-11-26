import { defineTest } from '@tests'
import { expect, vi } from 'vitest'
import { encode } from '@jridgewell/sourcemap-codec'
import { getLocation, getOutputAsset, getOutputChunk } from '@tests/utils'
import { SourceMapConsumer } from 'source-map'

// Copy from "rollup@sourcemaps@transform-low-resolution: handles combining low-resolution and high-resolution source-maps when transforming@generates es".
export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        transform(code) {
          // each entry of each line consist of
          // [generatedColumn, sourceIndex, sourceLine, sourceColumn];
          // this mapping only maps the second line to the first with no column
          // details
          const decodedMap = [[], [[0, 0, 0, 0]]]
          return {
            code: `console.log('added');\n${code}`,
            // @ts-expect-error typing is not same as rollup
            map: { mappings: encode(decodedMap) }, // The invalid map used `sourceIndex`, but not `sources` field
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

    const generatedLoc = getLocation(code, code.indexOf(`"baz"`))
    const originalLoc = smc.originalPositionFor(generatedLoc)

    expect(originalLoc.line).toBe(1)
    expect(originalLoc.column).toBe(0)
  },
})
