import { defineTest } from '@tests'
import { expect } from 'vitest'

let isComposingJs = false
export default defineTest({
  beforeTest(testKind) {
    isComposingJs = testKind === 'compose-js-plugin'
  },
  config: {
    output: {
      exports: 'named',
      format: 'iife',
      name: 'module',
      extend: false,
    },
  },
  afterTest: (output) => {
    isComposingJs
      ? expect(output.output[0].code).toMatchInlineSnapshot(`
      "var module = (function(exports) {

      "use strict";

      //#region main.js
      const main = "main";

      //#endregion
      Object.defineProperty(exports, 'main', {
        enumerable: true,
        get: function () {
          return main;
        }
      });
      return exports;
      })({});"
    `)
      : expect(output.output[0].code).toMatchInlineSnapshot(`
      "var module = (function(exports) {

      "use strict";

      //#region main.js
      const main = "main";

      //#endregion
      Object.defineProperty(exports, 'main', {
        enumerable: true,
        get: function () {
          return main;
        }
      });
      return exports;
      })({});"
    `)
  },
})
