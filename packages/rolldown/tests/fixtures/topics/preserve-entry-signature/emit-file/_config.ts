import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        buildStart(opt) {
          this.emitFile({
            type: 'chunk',
            id: './main.js',
            name: 'main',
            preserveSignature: false,
          });
        },
      },
    ],
    output: {},
  },
  afterTest: async (output) => {
    await import('./_test.mjs');
  },
});
