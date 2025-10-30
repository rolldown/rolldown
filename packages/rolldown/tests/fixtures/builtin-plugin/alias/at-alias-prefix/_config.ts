import { defineTest } from 'rolldown-tests';
import { aliasPlugin } from 'rolldown/experimental';

export default defineTest({
  config: {
    input: './main.js',
    plugins: [
      aliasPlugin({
        entries: [{ find: '@utils', replacement: './utils' }],
      }),
    ],
  },
  async afterTest() {
    await import('./assert.mjs');
  },
});
