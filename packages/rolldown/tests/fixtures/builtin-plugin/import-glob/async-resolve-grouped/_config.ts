import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { viteImportGlobPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

const resolveCalls: string[] = [];

export default defineTest({
  config: {
    plugins: [
      {
        name: 'async-import-glob-resolver',
        async resolveId(id) {
          if (!id.startsWith('#')) return;
          await Promise.resolve();
          resolveCalls.push(id);
          if (id === '#features/*.js') {
            return path.resolve(import.meta.dirname, 'features/*.js');
          }
          if (id === '#other/*.js') {
            return path.resolve(import.meta.dirname, 'other/*.js');
          }
        },
      },
      viteImportGlobPlugin(),
    ],
  },
  async afterTest() {
    expect(resolveCalls).toEqual(['#missing/*.js', '#features/*.js', '#other/*.js']);
    await import('./assert.mjs');
  },
});
