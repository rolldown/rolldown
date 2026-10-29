import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const IMPORTER_ID = '\0virtual:asset?width=100&height=200';

export default defineTest({
  config: {
    experimental: { importAttributes: true },
    plugins: [
      {
        name: 'resolver',
        async buildStart() {
          await this.resolve('./dep', IMPORTER_ID);
        },
      },
      {
        name: 'asserter',
        resolveId(source, importer, { importerRawId, importerAttributes }) {
          if (source === './dep') {
            expect(importer).toBe(IMPORTER_ID);
            expect(importerRawId).toBe(IMPORTER_ID);
            expect(importerAttributes).toEqual({});
            return false;
          }
        },
      },
    ],
  },
});
