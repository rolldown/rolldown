import { expect } from 'vitest'
import { getOutputFileNames } from 'rolldown-tests/utils'
import { defineTest } from 'rolldown-tests'

// TODO It is much better if defineTest can be an array of testsConfigs. Thus we can reuse same input for multiple tests.
export default defineTest({
  config: {
    input: ['main.js'],
    output: {
      sourcemap: true,
      sourcemapBaseUrl: 'https://example.com/foo/bar',
    },
  },
  afterTest: function (output) {
    expect(getOutputFileNames(output)).toStrictEqual(['main.js', 'main.js.map'])
    // include map comment
    expect(output.output[0].code).contains('//# sourceMappingURL=https://example.com/foo/bar/main.js.map')
    expect(output.output[0].sourcemapFileName).toBe('main.js.map')
    expect(output.output[0].map).toBeDefined()

    if (output.output[1].type === 'asset') {
      const map = JSON.parse(output.output[1].source.toString())
      expect(map.file).toMatch('main.js')
      expect(map.mappings).toMatchInlineSnapshot(
        `";AAAA,MAAa,MAAM;;;;ACEnB,QAAQ,IAAI,IAAI"`,
      )
    }
  },
})
