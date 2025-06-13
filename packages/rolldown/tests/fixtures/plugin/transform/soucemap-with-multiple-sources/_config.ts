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
        transform() {
          return {
            code: `
console.log('bar');

console.log('foo');
`.trim(),
            map: {
              version: 3,
              file: 'foo.js',
              sources: ['bar.js', 'foo.js'],
              sourcesContent: [
                "console.log('bar')",
                "import './bar'\nconsole.log('foo')",
              ],
              names: [],
              mappings:
                'AAAA,OAAO,CAAC,GAAG,CAAC,KAAK;;ACCjB,OAAO,CAAC,GAAG,CAAC,KAAK',
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

    const generatedLoc = getLocation(code, code.indexOf(`"bar"`))
    const originalLoc = smc.originalPositionFor(generatedLoc)

    expect(originalLoc.line).toBe(1)
    expect(originalLoc.column).toBe(12)
  },
})
