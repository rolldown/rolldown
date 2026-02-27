import { defineTest } from 'rolldown-tests';
import { expect, vi } from 'vitest';

const fn = vi.fn();
import { getOutputChunkNames } from 'rolldown-tests/utils';

export default defineTest({
  sequential: true,
  config: {
    input: ['main.js', 'entry.js'],
    plugins: [
      {
        name: 'virtual-module',
        resolveId(id) {
          if (id === '\0module') {
            return id;
          }
        },
        load(id) {
          if (id === '\0module') {
            fn();
            return `export default 'module'`;
          }
        },
      },
    ],
  },
  afterTest(output) {
    expect(JSON.stringify(getOutputChunkNames(output), null, 2)).toMatchInlineSnapshot(`
      "[
        "entry.js",
        "main.js",
        "_module-DFYPOykc.js"
      ]"
    `);
  },
});
