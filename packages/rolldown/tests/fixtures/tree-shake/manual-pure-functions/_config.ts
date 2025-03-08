import type { OutputChunk as RolldownOutputChunk } from 'rolldown'
import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    treeshake: {
      manualPureFunctions: ['styled', 'local'],
    },
    external: ['styled-components'],
  },
  afterTest: (output) => {
    output.output
      .filter(({ type }) => type === 'chunk')
      .forEach((chunk) => {
        let code = (chunk as RolldownOutputChunk).code

        expect(code).toMatchInlineSnapshot(`
          "import "styled-components";

          //#region main.js
          let another = console.log;
          another();

          //#endregion"
        `)
      })
  },
})
