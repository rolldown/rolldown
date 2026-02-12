import { defineTest } from 'rolldown-tests';
import { viteImportGlobPlugin } from 'rolldown/experimental';

export default defineTest({
  config: {
    input: './src/main.js',
    plugins: [viteImportGlobPlugin()],
  },
  async afterTest() {
    await import('./assert.mjs');
  },
});
