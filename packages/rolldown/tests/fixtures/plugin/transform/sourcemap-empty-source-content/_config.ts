import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

const originalCode = "console.log('original');\n";

export default defineTest({
  config: {
    input: ['empty.js', 'null.js', 'single.js'],
    plugins: [
      {
        name: 'test-plugin',
        transform(code, id) {
          if (!/\/(empty|null|single)\.js$/.test(id)) {
            return;
          }
          expect(code).toBe(originalCode);
          const single = id.endsWith('/single.js');
          return {
            code: 'console.log("a");\nexport const b = 2;',
            map: {
              version: 3,
              sources: single ? [id] : ['/a.ts', '/b.ts'],
              sourcesContent: single
                ? ['']
                : [id.endsWith('/empty.js') ? '' : null, 'export const b = 2'],
              names: [],
              mappings: single ? 'AAAA;AACA' : 'AAAA;ACAA',
            },
          };
        },
      },
    ],
    output: {
      sourcemap: true,
    },
  },
  afterTest(output) {
    const chunks = getOutputChunk(output);
    expect(chunks).toHaveLength(3);
    for (const chunk of chunks) {
      if (chunk.name === 'single') {
        expect(chunk.map?.sourcesContent).toEqual([originalCode]);
      } else {
        expect(chunk.map?.sourcesContent).toEqual([
          chunk.name === 'empty' ? '' : null,
          'export const b = 2',
        ]);
        expect(chunk.map?.sourcesContent?.[0]).not.toBe(originalCode);
      }
    }
  },
});
