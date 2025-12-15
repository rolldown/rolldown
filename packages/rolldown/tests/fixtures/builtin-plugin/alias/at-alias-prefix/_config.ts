import { defineTest } from 'rolldown-tests';
import { viteAliasPlugin } from 'rolldown/experimental';

export default defineTest({
  config: {
    input: './main.js',
    plugins: [
      viteAliasPlugin({
        entries: [{ find: '@utils', replacement: './utils' }],
      }),
    ],
  },
  async afterTest() {
    await import('./assert.mjs');
  },
});
