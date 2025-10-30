import { defineTest } from 'rolldown-tests';
import { transformPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: './main.ts',
    plugins: [
      transformPlugin(),
      {
        name: 'test',
        transform(code) {
          // after transform there should be no `interface`
          expect(code).not.include('interface');
          return null;
        },
      },
    ],
  },
  async afterTest() {
    await import('./assert.mjs');
  },
});
