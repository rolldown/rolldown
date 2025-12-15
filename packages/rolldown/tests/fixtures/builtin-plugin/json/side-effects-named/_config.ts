import { defineTest } from 'rolldown-tests';
import { viteJsonPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  config: {
    plugins: [
      viteJsonPlugin({
        minify: false,
        stringify: true,
        namedExports: true,
      }),
    ],
  },
  async afterTest(output) {
    expect(output.output[0].code).not.toContain(`JSON.parse`);
    await import('./assert.mjs');
  },
});
