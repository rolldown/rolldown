import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { viteAliasPlugin } from 'rolldown/experimental';

const appDir = path.resolve(import.meta.dirname, 'src/app').replaceAll(path.sep, '/');

export default defineTest({
  config: {
    input: './main.js',
    plugins: [
      viteAliasPlugin({
        entries: [
          {
            find: /^@app(?!\/(?:excluded))(\/.*)?$/,
            replacement: appDir + '$1',
          },
        ],
      }),
    ],
  },
  async afterTest() {
    await import('./assert.mjs');
  },
});
