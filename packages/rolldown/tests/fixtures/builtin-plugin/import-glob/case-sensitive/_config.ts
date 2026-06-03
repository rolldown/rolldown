import { defineTest } from 'rolldown-tests';
import { viteImportGlobPlugin } from 'rolldown/experimental';

export default defineTest({
  config: {
    plugins: [viteImportGlobPlugin()],
  },
  async afterTest() {
    await import('./assert.mjs');
  },
});
