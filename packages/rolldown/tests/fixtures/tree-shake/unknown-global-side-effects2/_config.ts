import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    treeshake: {
      unknownGlobalSideEffects: false,
    },
  },
  afterTest: (output) => {
    let code = output.output[0].code

    expect(code).toMatchInlineSnapshot(`
      "//#region main.js
      const element = angular.element;

      //#endregion"
    `)
  },
})
