import type { OutputChunk as RolldownOutputChunk } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    output: {
      hashCharacters: 'hex',
      entryFileNames: '[name]-[hash]-[hash:6].js',
      chunkFileNames: '[name]-[hash]-[hash:7].js',
    },
  },
  afterTest: (output) => {
    const hash_entry =
      output.output
        .find((chunk) => (chunk as RolldownOutputChunk).isEntry)
        ?.fileName.match(/-([a-f0-9]+)-([a-f0-9]+)\.js$/) || [];
    const hash_chunk =
      output.output
        .find((chunk) => !(chunk as RolldownOutputChunk).isEntry)
        ?.fileName.match(/-([a-f0-9]+)-([a-f0-9]+)\.js$/) || [];

    expect(hash_entry[1]).toHaveLength(8);
    expect(hash_entry[2]).toHaveLength(6);
    expect(hash_chunk[1]).toHaveLength(8);
    expect(hash_chunk[2]).toHaveLength(7);
  },
});
