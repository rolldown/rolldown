import { defineTest } from 'rolldown-tests';
import { loadFallbackPlugin } from 'rolldown/experimental';

export default defineTest({
  config: {
    plugins: [loadFallbackPlugin()],
  },
  async afterTest() {
    await import('./assert.mjs');
  },
});
