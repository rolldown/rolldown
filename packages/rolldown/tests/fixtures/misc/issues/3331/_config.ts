import { defineTest } from 'rolldown-tests';

export default defineTest({
  config: {
    output: {
      strictExecutionOrder: true,
    },
  },
  afterTest: async () => {
    await import('./assert.mjs');
  },
});
