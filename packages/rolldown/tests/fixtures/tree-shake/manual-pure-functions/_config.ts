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
    let code = output.output[0].code

    expect(code).toMatchInlineSnapshot(`
      "import "styled-components";

      //#region main.js
      let another = console.log;
      another();
      "
    `)
  },
})
