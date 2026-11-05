import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { viteImportGlobPlugin } from 'rolldown/experimental';

export default defineTest({
  config: {
    input: './app/main.js',
    plugins: [viteImportGlobPlugin({ root: path.join(import.meta.dirname, 'app') })],
  },
  async afterTest() {
    await import('./assert.mjs');
  },
});
