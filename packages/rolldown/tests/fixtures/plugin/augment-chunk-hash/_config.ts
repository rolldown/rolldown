import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect, vi } from 'vitest';

const fn = vi.fn();

export default defineTest({
  sequential: true,
  config: {
    input: ['main.js', 'entry.js'],
    output: {
      entryFileNames: '[name]-[hash].js',
      chunkFileNames: '[name]-[hash].js',
    },
    plugins: [
      {
        name: 'test-plugin',
        augmentChunkHash: (chunk) => {
          fn();
          if (chunk.fileName.includes('entry')) {
            expect(Object.values(chunk.modules)[0].code).toBe('// entry.js\nconsole.log();\n');
            expect(Object.values(chunk.modules)[0].renderedLength).toBe(27);
            return 'entry-hash';
          }
        },
      },
    ],
  },
  afterTest: (output) => {
    expect(fn).toHaveBeenCalledTimes(2);
    const chunks = getOutputChunk(output);
    for (const chunk of chunks) {
      switch (chunk.facadeModuleId) {
        case path.join(__dirname, 'main.js'):
          expect(chunk.fileName).toMatchInlineSnapshot(`"main-D1A2dxke.js"`);
          break;

        case path.join(__dirname, 'entry.js'):
          expect(chunk.fileName).toMatchInlineSnapshot(`"entry-C_K2JJpP.js"`);
          break;

        default:
          break;
      }
    }
  },
});
