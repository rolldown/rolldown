import { defineTest } from 'rolldown-tests';
import { viteLoadFallbackPlugin } from 'rolldown/experimental';

export default defineTest({
  config: {
    plugins: [viteLoadFallbackPlugin()],
  },
  async afterTest() {
    await import('./assert.mjs');
  },
});
