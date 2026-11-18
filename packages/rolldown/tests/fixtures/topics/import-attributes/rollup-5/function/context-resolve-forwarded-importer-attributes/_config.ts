import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const compositeImporter = `importer?attributes=${encodeURIComponent(
  JSON.stringify({ type: 'json' }),
)}`;

export default defineTest({
  config: {
    experimental: { importAttributes: true },
    plugins: [
      {
        name: 'caller',
        async buildStart() {
          await this.resolve('target', compositeImporter, {
            // @ts-expect-error Rolldown has no `importerAttributes` option, Rollup 5 deprecates it
            importerAttributes: { type: 'json' },
          });
        },
      },
      {
        name: 'forwarder',
        async resolveId(source, importer, options) {
          if (source === 'target') {
            return this.resolve('nested-target', importer, options);
          }
        },
      },
      {
        name: 'observer',
        resolveId(source, importer, { importerAttributes, importerRawId }) {
          if (source === 'nested-target') {
            expect(importer).toBe(compositeImporter);
            expect(importerRawId).toBe('importer');
            expect(importerAttributes).toEqual({ type: 'json' });
            return { external: true, id: source };
          }
        },
      },
    ],
  },
});
