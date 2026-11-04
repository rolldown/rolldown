import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    experimental: { importAttributes: true },
    plugins: [
      {
        name: 'test',
        async buildStart() {
          await this.resolve(
            'target',
            { attributes: { type: 'object' }, rawId: 'importer' },
            {
              // @ts-expect-error Rolldown has no `importerAttributes` option, Rollup 5 deprecates it
              importerAttributes: { type: 'option' },
            },
          );
        },
      },
      {
        name: 'observer',
        resolveId(source, importer, { importerAttributes, importerRawId }) {
          if (source === 'target') {
            expect(importer).toBe('importer?attributes=%7B%22type%22%3A%22object%22%7D');
            expect(importerRawId).toBe('importer');
            expect(importerAttributes).toEqual({ type: 'object' });
          }
        },
      },
    ],
  },
});
