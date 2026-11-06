import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const encodedAttributes = '%7B%22language%22%3A%22en%22%2C%22type%22%3A%22json%22%7D';
const expectedRawIdsByImporter = new Map([
  [`importer?attributes=${encodedAttributes}`, 'importer'],
  [`importer?plugin&attributes=${encodedAttributes}`, 'importer?plugin'],
]);

export default defineTest({
  config: {
    experimental: { importAttributes: true },
    plugins: [
      {
        name: 'test',
        async buildStart() {
          for (const importer of expectedRawIdsByImporter.keys()) {
            await this.resolve('target', importer, { skipSelf: false });
          }
        },
        resolveId(id, importer, { attributes, importerAttributes, importerRawId }) {
          if (id === 'main') return id;
          if (id === 'target') {
            expect(importerAttributes).toEqual({ language: 'en', type: 'json' });
            expect(attributes).toEqual({});
            expect(importerRawId).toBe(expectedRawIdsByImporter.get(importer!));
            return { external: true, id };
          }
          return {
            attributes: { type: 'json', language: 'en' },
            id: 'module?plugin',
          };
        },
        load(id, { attributes, rawId }) {
          if (id === 'main') return "import 'module';";
          expect(id).toBe(`module?plugin&attributes=${encodedAttributes}`);
          expect(rawId).toBe('module?plugin');
          expect(attributes).toEqual({ language: 'en', type: 'json' });
          // Rolldown gives `type: 'json'` its built-in meaning, so the module has to be JSON.
          return '{}';
        },
      },
    ],
  },
});
