import { defineTest } from 'rolldown-tests';
import { viteImportGlobPlugin } from 'rolldown/experimental';

export default defineTest({
  // Skipping this test for now to align with vite
  skip: true,
  config: {
    input: './src/main.js',
    plugins: [viteImportGlobPlugin()],
  },
  async afterTest() {
    await import('./assert.mjs');
  },
});
