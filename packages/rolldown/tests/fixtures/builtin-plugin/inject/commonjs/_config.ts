import nodePath from 'node:path';
import { defineTest } from 'rolldown-tests';
import { injectPlugin } from 'rolldown/plugins';

const resolve = (file: string) => nodePath.join(import.meta.dirname, file);

export default defineTest({
  config: {
    external: ['node:assert'],
    plugins: [
      injectPlugin({
        useKey: [resolve('composable.js'), 'useKey'],
        'Object.getPrototypeOf': resolve('get-prototype-of.js'),
      }),
    ],
  },
  afterTest: async () => {
    await import('./_test.mjs');
  },
});
