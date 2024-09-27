import { defineTest } from '@tests'
import { expect } from 'vitest'
let isComposingJs = false
export default defineTest({
  beforeTest(testKind) {
    isComposingJs = testKind === 'compose-js-plugin'
  },
  config: {
    external: /node:path/,
    output: {
      exports: 'named',
      format: 'iife',
    },
  },
  afterTest: (output) => {
    isComposingJs
      ? expect(output.output[0].code).toMatchInlineSnapshot(`
      "(function(exports, node_path) {

      "use strict";
      Object.defineProperty(exports, '__esModule', { value: true });
      const { join } = node_path;

      //#region main.js
      var main_default = join;

      //#endregion
      Object.defineProperty(exports, 'default', {
        enumerable: true,
        get: function () {
          return main_default;
        }
      });
      return exports;
      })({}, node_path);"
    `)
      : expect(output.output[0].code).toMatchInlineSnapshot(`
      "(function(exports, node_path) {

      "use strict";
      Object.defineProperty(exports, '__esModule', { value: true });
      const { join } = node_path;

      //#region main.js
      var main_default = join;

      //#endregion
      Object.defineProperty(exports, 'default', {
        enumerable: true,
        get: function () {
          return main_default;
        }
      });
      return exports;
      })({}, node_path);"
    `)
  },
})
