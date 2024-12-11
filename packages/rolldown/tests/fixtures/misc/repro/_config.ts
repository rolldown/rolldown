import { defineTest } from '@tests'
import { getLocation } from '@tests/utils'
import { SourceMapConsumer } from 'source-map'
import { expect } from 'vitest'

export default defineTest({
  config: {
    output: {
      sourcemap: 'inline',
    },
    plugins: [
      {
        name: 'repro',
        transform(_code, id) {
          if (id.includes('dep2')) {
            return {
              code: `export default "dep2-generated"`,
              map: { mappings: '' },
            }
          }
        },
      },
    ],
  },
  async afterTest(output) {
    const chunk = output.output[0]
    const code = chunk.code
    const smc = await new SourceMapConsumer(chunk.map!)
    expect([
      smc.originalPositionFor(getLocation(code, code.indexOf(`dep1-test`))),
      smc.originalPositionFor(
        getLocation(code, code.indexOf(`dep2-generated`)),
      ),
      smc.originalPositionFor(getLocation(code, code.indexOf(`dep3-test`))),
    ]).toMatchInlineSnapshot(`
      [
        {
          "column": 15,
          "line": 1,
          "name": null,
          "source": "../dep1.js",
        },
        {
          "column": null,
          "line": null,
          "name": null,
          "source": null,
        },
        {
          "column": 15,
          "line": 1,
          "name": null,
          "source": "../dep3.js",
        },
      ]
    `)
  },
})
