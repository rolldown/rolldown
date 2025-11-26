import { defineTest } from 'rolldown-tests';

export default defineTest({
  config: {
    input: ['./src/bar.tsx', './src/foo.tsx'],
    tsconfig: true,
  },
  async afterTest() {
    await import('./assert.mjs');
  },
});
