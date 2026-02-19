import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const entry = path.join(__dirname, './main.js');

export default defineTest({
  config: {
    input: entry,
    plugins: [
      {
        name: 'test-plugin',
        resolveId: function (id, _importer, _options) {
          if (id === 'external') {
            return {
              id,
              external: true,
              moduleSideEffects: false,
            };
          }
        },
      },
    ],
  },
  afterTest: (output) => {
    expect(output.output[0].code.includes('external')).toBe(false);
  },
});
