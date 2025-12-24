import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: ['./main.js', './entry.js'],
  },
  afterTest(output) {
    const mainChunk = output.output.find((chunk) =>
      chunk.fileName === 'main.js'
    );
    const entry2Chunk = output.output.find((chunk) =>
      chunk.fileName === 'entry.js'
    );

    expect(mainChunk).toBeDefined();
    expect(entry2Chunk).toBeDefined();

    if (mainChunk?.type === 'chunk' && entry2Chunk?.type === 'chunk') {
      // main.js imports entry.js, so main.js should have 'entry.js' in its imports
      expect(mainChunk.imports).toContain('entry.js');
      // entry.js has no imports
      expect(entry2Chunk.imports).toEqual([]);
    }
  },
});
