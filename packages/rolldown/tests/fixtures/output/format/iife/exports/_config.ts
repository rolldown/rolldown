import { defineTest } from '@tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    external: /node:path/,
    output: {
      exports: 'named',
      format: 'iife',
    },
  },
  afterTest: (output) => {
    expect(output.output[0].code).toMatchInlineSnapshot(`
      "(function(exports, node_path) {

      "use strict";

      const { join } = node_path;

      //#region main.js
      var main_default = join;

      //#endregion
      Object.defineProperty(exports, '__esModule', { value: true });
      exports.default = main_default;
      return exports;
      })({}, node_path);
      "
    `)
  },
})
