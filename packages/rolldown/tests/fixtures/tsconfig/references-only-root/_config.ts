import { defineTest } from 'rolldown-tests';

export default defineTest({
  config: {
    input: ['./src/app/index.tsx', './src/server/index.ts'],
    tsconfig: true,
  },
  async afterTest() {
    await import('./assert.mjs');
  },
});
