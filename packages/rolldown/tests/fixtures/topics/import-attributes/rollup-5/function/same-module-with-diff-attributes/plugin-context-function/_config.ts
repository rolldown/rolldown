import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const fooSource = './foo.js';

// Rollup 5 also emits a chunk with attributes and calls `this.getModuleInfo({ rawId, attributes })`.
// Rolldown supports neither yet, so those parts are left out.
export default defineTest({
  config: {
    input: path.join(import.meta.dirname, 'main.js'),
    experimental: { importAttributes: true },
    plugins: [
      {
        name: 'first',
        resolveId(id, _importer, { importerRawId, attributes }) {
          if (id === fooSource) {
            return this.resolve(
              id,
              { rawId: importerRawId!, attributes: { type: 'main' } },
              { attributes },
            );
          }
          return id;
        },
        async load(_id, { rawId, attributes }) {
          if (attributes?.type === 'unchanged') {
            const info = await this.load({
              id: {
                rawId,
                attributes: {
                  type: 'changed',
                },
              },
            });
            return info.code;
          }
          if (attributes?.type === 'changed') {
            return 'export const foo = "changed";';
          }
        },
      },
      {
        name: 'second',
        resolveId(id, importer) {
          expect(importer!.endsWith('main.js?attributes=%7B%22type%22%3A%22main%22%7D')).toBe(true);
          return id;
        },
      },
    ],
  },
  async afterTest() {
    // @ts-ignore
    const { foo } = await import('./dist/main.js');
    expect(foo).toBe('changed');
  },
});
