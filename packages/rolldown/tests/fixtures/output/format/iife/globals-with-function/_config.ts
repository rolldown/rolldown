import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    external: /node:path/,
    output: {
      format: 'iife',
      name: 'module',
      globals: (name: string): string => {
        if (name === 'node:path') {
          return 'path';
        }

        return '';
      },
    },
  },
  afterTest: (output) => {
    expect(output.output[0].code).toMatchInlineSnapshot(`
      "var module = (function(node_path) {
      	//#endregion
      	return node_path.join;
      })(path);
      "
    `);
  },
});
