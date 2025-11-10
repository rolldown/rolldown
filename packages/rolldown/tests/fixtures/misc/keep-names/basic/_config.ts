import { defineTest } from 'rolldown-tests';

export default defineTest({
  config: {
    external: ['node:assert'],
    output: {
      keepNames: true,
    },
  },
  afterTest: async () => {
    await import('./assert.mjs');
  },
});
