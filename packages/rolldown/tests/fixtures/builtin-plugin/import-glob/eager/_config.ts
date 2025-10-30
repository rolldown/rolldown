import { defineTest } from 'rolldown-tests';
import { importGlobPlugin } from 'rolldown/experimental';

export default defineTest({
  config: {
    plugins: [importGlobPlugin()],
  },
  async afterTest() {
    await import('./assert.mjs');
  },
});
