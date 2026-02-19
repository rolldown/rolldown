import { defineTest } from 'rolldown-tests';

export default defineTest({
  config: {
    output: {
      preserveModules: true,
    },
  },
  afterTest: async (_output) => {
    await import('./_test.mjs');
  },
});
