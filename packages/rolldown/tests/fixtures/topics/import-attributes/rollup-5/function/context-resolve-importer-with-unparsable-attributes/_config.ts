import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const opaqueImporters = [
  'virtual?attributes=foo',
  'virtual?attributes=%zz',
  `foo&attributes=${encodeURIComponent(JSON.stringify({ type: 'json' }))}`,
  'virtual?attributes=%7B%7D',
];
const appendedAttributesImporter = `virtual?attributes=foo&attributes=${encodeURIComponent(
  JSON.stringify({ type: 'json' }),
)}`;

export default defineTest({
  config: {
    experimental: { importAttributes: true },
    plugins: [
      {
        name: 'caller',
        async buildStart() {
          for (const importer of [...opaqueImporters, appendedAttributesImporter]) {
            const resolution = await this.resolve('target', importer);
            expect(resolution!.id).toBe('target');
            expect(resolution!.external).toBe(true);
          }
        },
      },
      {
        name: 'resolver',
        resolveId(source, importer, { importerAttributes, importerRawId }) {
          if (source !== 'target') return;
          if (opaqueImporters.includes(importer!)) {
            expect(importerRawId).toBe(importer);
            expect(importerAttributes).toEqual({});
          } else {
            expect(importer).toBe(appendedAttributesImporter);
            expect(importerRawId).toBe('virtual?attributes=foo');
            expect(importerAttributes).toEqual({ type: 'json' });
          }
          return { external: true, id: source };
        },
      },
    ],
  },
});
