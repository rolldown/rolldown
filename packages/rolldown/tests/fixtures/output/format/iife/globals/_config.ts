import { defineTest } from '@tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    external: /node:path/,
    output: {
      format: 'iife',
      globals: {
        'node:path': 'path',
      },
    },
  },
  afterTest: (output) => {
    expect(output.output[0].code).toMatchInlineSnapshot(`
      "(function(node_path) {
      "use strict";
      const { join } = node_path;

      //#region main.js
      var main_default = join;

      //#endregion
      return main_default;
      })(node_path);"
    `)
  },
})
