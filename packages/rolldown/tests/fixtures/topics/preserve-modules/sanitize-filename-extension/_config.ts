import type { OutputChunk } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// Like Rollup, `sanitizeFileName` gets the relative path with its extension, and only a JS/TS
// extension of the result is removed. `toUpperCase` turns `.js` into `.JS`, so the names keep it.
export default defineTest({
  config: {
    input: ['./index.js'],
    output: {
      preserveModules: true,
      sanitizeFileName: (name) => name.toUpperCase(),
    },
  },
  afterTest: (output) => {
    const chunks = output.output.filter((item): item is OutputChunk => item.type === 'chunk');
    expect(Object.fromEntries(chunks.map((chunk) => [chunk.fileName, chunk.name]))).toStrictEqual({
      'INDEX.JS.js': 'INDEX.JS',
      'FOO.JS.js': 'FOO.JS',
    });
  },
});
