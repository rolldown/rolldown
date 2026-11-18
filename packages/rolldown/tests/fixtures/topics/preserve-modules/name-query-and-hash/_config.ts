import path from 'node:path';
import type { OutputChunk } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// Like Rollup, the name of a preserved module ignores the id after its first `?` or `#`. It
// keeps an extension that is not a JS one, and a trailing dot. With `sanitizeFileName: false`,
// a `?` or `#` in a file name would make the import in Node fail.
const ids: Record<string, string> = {
  data: path.join(import.meta.dirname, 'data.json') + '?raw',
  x: path.join(import.meta.dirname, 'x.js') + '#a/b',
  card: path.join(import.meta.dirname, 'card.'),
};

export default defineTest({
  sequential: true,
  config: {
    output: {
      dir: 'dist',
      preserveModules: true,
      entryFileNames: '[name].mjs',
      sanitizeFileName: false,
    },
    plugins: [
      {
        name: 'virtual-query-and-hash',
        resolveId(id) {
          return ids[id];
        },
        load(id) {
          const key = Object.keys(ids).find((key) => ids[key] === id);
          if (key) {
            return `(globalThis.nameQueryAndHash ??= []).push('${key}');\nexport default '${key}';`;
          }
        },
      },
    ],
  },
  afterTest: async (output) => {
    const chunks = output.output.filter((item): item is OutputChunk => item.type === 'chunk');
    expect(Object.fromEntries(chunks.map((chunk) => [chunk.fileName, chunk.name]))).toStrictEqual({
      'card..mjs': 'card.',
      'data.json.mjs': 'data.json',
      'main.mjs': 'main',
      'x.mjs': 'x',
    });
    await import('./_test.mjs');
  },
});
