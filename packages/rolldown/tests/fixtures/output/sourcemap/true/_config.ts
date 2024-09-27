// cSpell:disable
import { expect } from 'vitest'
import { getOutputFileNames } from '@tests/utils'
import { defineTest } from '@tests'
let isComposingJs = false
export default defineTest({
  beforeTest(testKind) {
    isComposingJs = testKind === 'compose-js-plugin'
  },
  config: {
    input: ['main.js'],
    output: {
      sourcemap: true,
    },
  },
  afterTest: function (output) {
    expect(getOutputFileNames(output)).toStrictEqual(['main.js', 'main.js.map'])
    // include map comment
    expect(output.output[0].code).contains('//# sourceMappingURL=main.js.map')
    expect(output.output[0].sourcemapFileName).toBe('main.js.map')
    expect(output.output[0].map).toBeDefined()

    if (output.output[1].type === 'asset') {
      const map = JSON.parse(output.output[1].source.toString())
      isComposingJs
        ? expect(map.file).toMatchInlineSnapshot(`"main.js"`)
        : expect(map.file).toMatchInlineSnapshot(`"main.js"`)
      isComposingJs
        ? expect(map.mappings).toMatchInlineSnapshot(
            `";;AAAA,MAAa,MAAM;;;;ACEnB,QAAQ,IAAI,IAAI"`,
          )
        : expect(map.mappings).toMatchInlineSnapshot(
            `";;AAAA,MAAa,MAAM;;;;ACEnB,QAAQ,IAAI,IAAI"`,
          )
    }
  },
})
