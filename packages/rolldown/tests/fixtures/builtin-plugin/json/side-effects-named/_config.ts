import { defineTest } from 'rolldown-tests';
import { jsonPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  config: {
    plugins: [
      jsonPlugin({
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
