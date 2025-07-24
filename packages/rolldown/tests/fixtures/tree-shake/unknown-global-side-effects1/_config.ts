import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'

export default defineTest({
  afterTest: (output) => {
    let code = output.output[0].code

    expect(code).toMatchInlineSnapshot(`
      "//#region main.js
      $;
      angular.element;

      //#endregion"
    `)
  },
})
