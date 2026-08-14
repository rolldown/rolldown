import fs from 'node:fs';
import { defineTest } from 'rolldown-tests';

export default defineTest({
  config: {
    input: './main.typescript',
    plugins: [
      {
        name: 'rewrite-module-type',
        load: function (id) {
          return {
            code: fs.readFileSync(id, 'utf-8'),
            moduleType: 'ts',
          };
        },
      },
    ],
  },
  afterTest: async (_output) => {
    await import('./assert.mjs');
  },
});
