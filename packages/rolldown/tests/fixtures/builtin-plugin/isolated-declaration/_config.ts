import { isolatedDeclarationPlugin } from 'rolldown/experimental'
import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'
import { getOutputFileNames } from 'rolldown-tests/utils'

export default defineTest({
  config: {
    input: 'main.ts',
    plugins: [
      isolatedDeclarationPlugin({
        stripInternal: true,
      }),
    ],
  },
  async afterTest(output) {
    expect(getOutputFileNames(output)).toMatchInlineSnapshot(`
      [
        "foo.d.ts",
        "main.d.ts",
        "main.js",
      ]
    `)
  },
})
