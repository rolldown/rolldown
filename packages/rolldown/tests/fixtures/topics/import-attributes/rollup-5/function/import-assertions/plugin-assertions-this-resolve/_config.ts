import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// Rolldown's `ResolvedId` has other fields than Rollup's, e.g. no `resolvedBy`, so only the
// fields this test is about are compared.
export default defineTest({
  config: {
    experimental: { importAttributes: true },
    plugins: [
      {
        name: 'first',
        async resolveId() {
          expect(
            await this.resolve('external', undefined, {
              attributes: { a: 'c', b: 'd' },
            }),
          ).toMatchObject({
            attributes: { a: 'changed', b: 'changed' },
            external: true,
            id: 'external',
            meta: {},
          });
        },
      },
      {
        name: 'second',
        async resolveId(source, importer, { attributes }) {
          if (source === 'external') {
            return this.resolve(source, importer, { attributes });
          }
        },
      },
      {
        name: 'third',
        async resolveId(source, _importer, { attributes }) {
          if (source === 'external') {
            return {
              id: source,
              attributes: Object.fromEntries(
                Object.keys(attributes).map((key) => [key, 'changed']),
              ),
              external: true,
            };
          }
        },
      },
    ],
  },
});
