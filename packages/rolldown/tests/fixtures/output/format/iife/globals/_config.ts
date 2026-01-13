import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
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
    expect(output.output[0].code).toMatchInlineSnapshot(`
      "var module = (function(node_path) {


      //#region main.js
      	var main_default = node_path.join;

      //#endregion
      return main_default;
      })(path);"
    `);
  },
});
