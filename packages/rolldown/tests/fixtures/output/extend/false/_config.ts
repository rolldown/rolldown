import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    output: {
      exports: 'named',
      format: 'iife',
      name: 'module',
      extend: false,
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
