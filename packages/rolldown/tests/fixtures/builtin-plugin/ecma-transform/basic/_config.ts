import { defineTest } from 'rolldown-tests';
import { viteTransformPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: './main.ts',
    plugins: [
      viteTransformPlugin(),
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
