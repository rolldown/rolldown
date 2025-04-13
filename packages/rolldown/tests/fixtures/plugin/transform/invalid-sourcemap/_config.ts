import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'
import { SourceMapConsumer } from 'source-map'
import {
  getLocation,
  getOutputAsset,
  getOutputChunk,
} from 'rolldown-tests/utils'

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
          const encode = (_map = decodedMap) => ';AAAA';
          return {
            code: `console.log('added');\n${code}`,
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
