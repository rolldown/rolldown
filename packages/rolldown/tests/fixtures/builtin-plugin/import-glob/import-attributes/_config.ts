import { viteImportGlobPlugin } from 'rolldown/experimental';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    experimental: { importAttributes: true },
    plugins: [
      viteImportGlobPlugin(),
      {
        name: 'check-glob-attributes',
        resolveId(source, _importer, { attributes, kind }) {
          if (source === './files/module.js') {
            expect(attributes).toEqual({
              type: 'text',
              'custom-key': kind === 'import-statement' ? 'eager' : 'a"b\\c\n',
            });
          }
        },
      },
    ],
  },
  async afterTest() {
    await import('./assert.mjs');
  },
});
