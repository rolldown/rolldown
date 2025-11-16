import { defineTest } from 'rolldown-tests';
import { viteJsonPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  config: {
    plugins: [
      viteJsonPlugin({ stringify: false, minify: true, namedExports: true }),
    ],
  },
  async afterTest(output) {
    expect(output.output[0].code).toContain(
      `const name = "@test-fixture/named-exports";`,
    );
    await import('./assert.mjs');
  },
});
