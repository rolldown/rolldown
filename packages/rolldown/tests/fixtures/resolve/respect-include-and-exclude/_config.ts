import { defineTest } from 'rolldown-tests';

export default defineTest({
  config: {
    input: ['./src/bar.ts', './src/foo.ts'],
    tsconfig: './src/tsconfig.json',
  },
  async afterTest() {
    await import('./assert.mjs');
  },
});
