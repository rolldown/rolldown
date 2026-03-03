import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: 'main.js',
    output: {
      entryFileNames: '[name].js',
      chunkFileNames: 'chunks/[name].js',
    },
    plugins: [
      {
        name: 'test-plugin',
        buildStart() {
          this.emitFile({
            type: 'chunk',
            name: 'emitted',
            id: path.resolve(__dirname, './emitted.js'),
          });
        },
      },
    ],
  },
  afterTest: (output) => {
    // The emitted chunk should use chunkFileNames pattern, not entryFileNames
    const emittedChunk = output.output.find((chunk) => chunk.name === 'emitted');
    expect(emittedChunk).toBeTruthy();
    // Should be in 'chunks/' directory per chunkFileNames pattern
    expect(emittedChunk!.fileName).toBe('chunks/emitted.js');

    // The regular entry should use entryFileNames pattern
    const mainChunk = output.output.find((chunk) => chunk.name === 'main');
    expect(mainChunk).toBeTruthy();
    // Should be in root per entryFileNames pattern
    expect(mainChunk!.fileName).toBe('main.js');
  },
});
