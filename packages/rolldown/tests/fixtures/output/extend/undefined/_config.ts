import { defineTest } from '@tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    output: {
      exports: 'named',
      format: 'iife',
      name: 'module',
    },
  },
  afterTest: (output) => {
    expect(output.output[0].code).toMatchInlineSnapshot(`
      "var module = (function(exports) {

      "use strict";

      //#region main.js
      const main = "main";

      //#endregion
      exports.main = main
      return exports;
      })({});"
    `)
  },
})
