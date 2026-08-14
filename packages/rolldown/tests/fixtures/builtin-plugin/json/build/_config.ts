import { defineTest } from 'rolldown-tests';
import { viteJsonPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  config: {
    plugins: [viteJsonPlugin({ stringify: true, minify: true })],
  },
  async afterTest(output) {
    expect(output.output[0].code).toContain(`JSON.parse("{\\"name\\":\\"@test-fixture/build\\"`);
    await import('./assert.mjs');
  },
});
