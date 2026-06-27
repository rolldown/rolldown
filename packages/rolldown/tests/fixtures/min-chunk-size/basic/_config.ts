import { expect } from 'vitest';
import { defineTest } from 'rolldown-tests';

// `experimental.minChunkSize`: two entries share a tiny side-effect-free leaf
// (`util.js`). Without the option rolldown emits 3 chunks (entry-a, entry-b, and
// a standalone shared chunk for util.js). With minChunkSize large enough, the
// leaf is duplicated into each entry chunk instead, yielding 2 chunks and no
// cross-chunk import.
export default defineTest({
  config: {
    input: {
      'entry-a': './entry-a.js',
      'entry-b': './entry-b.js',
    },
    experimental: {
      minChunkSize: 100_000,
    },
  },
  afterTest: async (output) => {
    const chunks = output.output.filter((c) => c.type === 'chunk');
    // Shared leaf merged into both entries -> 2 chunks, no standalone shared chunk.
    expect(chunks.length).toBe(2);
    // util.js is duplicated into BOTH entry chunks.
    const chunksWithUtil = chunks.filter((c) =>
      c.moduleIds.some((id) => id.replace(/\\/g, '/').endsWith('/util.js')),
    );
    expect(chunksWithUtil.length).toBe(2);
    // No cross-chunk import remains between the entries.
    for (const c of chunks) {
      expect(c.imports.length).toBe(0);
    }
    // Runtime correctness: the duplicated leaf's bindings resolve in each chunk.
    const a = await import('./dist/entry-a.js');
    const b = await import('./dist/entry-b.js');
    expect(a.a).toBe('shared-value-a');
    expect(b.b).toBe('shared-valueshared-value-b');
  },
});
