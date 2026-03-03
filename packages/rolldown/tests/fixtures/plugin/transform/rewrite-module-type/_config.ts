import { defineTest } from 'rolldown-tests';

export default defineTest({
  config: {
    input: './main.typescript',
    plugins: [
      {
        name: 'rewrite-module-type',
        transform: function (_code, _id, _meta) {
          return {
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
