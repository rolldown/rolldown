import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { viteImportGlobPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

let resolveCalls = 0;

export default defineTest({
  config: {
    plugins: [
      {
        name: 'async-import-glob-resolver',
        async resolveId(id) {
          if (id === '#features/*.js') {
            await Promise.resolve();
            resolveCalls += 1;
            return path.resolve(
              import.meta.dirname,
              resolveCalls === 1 ? 'features/*.js' : 'other/*.js',
            );
          }
        },
      },
      viteImportGlobPlugin(),
    ],
  },
  async afterTest() {
    expect(resolveCalls).toBe(2);
    await import('./assert.mjs');
  },
});
