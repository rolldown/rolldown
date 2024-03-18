// cSpell:disable
import type { RollupOptions, RollupOutput } from '../../../../src'
import path from 'node:path'
import { expect } from 'vitest'
import { getOutputFileNames } from '../../../util'

const config: RollupOptions = {
  input: [path.join(__dirname, 'main.js')],
  output: {
    sourcemap: true,
  },
}

export default {
  config,
  afterTest: function (output: RollupOutput) {
    expect(getOutputFileNames(output)).toMatchInlineSnapshot(`
      [
        "main.js",
        "main.js.map",
      ]
    `)
    // include map comment
    expect(output.output[0].code).contains('//# sourceMappingURL=main.js.map')
    // @ts-expect-error
    const map = JSON.parse(output.output[1].source)
    expect(map.sources).toMatchInlineSnapshot(`
      [
        "test/cases/sourcemap/inner/foo.js",
        "test/cases/sourcemap/inner/main.js",
      ]
    `)
    expect(map.file).toMatchInlineSnapshot(`"main.js"`)
    expect(map.mappings).toMatchInlineSnapshot(
      `";;AAAO,MAAMA,MAAM;;;ACEnB,QAAQ,IAAIA,IAAI"`,
    )
  },
}
