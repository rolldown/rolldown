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
      format: 'iife',
      name: 'module',
      globals: {
        'node:path': 'path',
      },
    },
  },
  afterTest: (output) => {
    isComposingJs
      ? expect(output.output[0].code).toMatchInlineSnapshot(`
      "var module = (function(node_path) {

      "use strict";
      const { join } = node_path;

      //#region main.js
      var main_default = join;

      //#endregion
      return main_default;
      })(path);"
    `)
      : expect(output.output[0].code).toMatchInlineSnapshot(`
      "var module = (function(node_path) {

      "use strict";
      const { join } = node_path;

      //#region main.js
      var main_default = join;

      //#endregion
      return main_default;
      })(path);"
    `)
  },
})
