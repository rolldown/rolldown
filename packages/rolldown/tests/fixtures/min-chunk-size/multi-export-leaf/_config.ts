import { expect } from 'vitest';
import { defineTest } from 'rolldown-tests';

// A multi-export leaf shared by THREE entries, with minify ON. Verifies that the
// duplicated leaf's globally-pinned names stay consistent within each chunk after
// minification (def + references both rename together), with no cross-chunk import.
export default defineTest({
  config: {
    input: {
      e1: './e1.js',
      e2: './e2.js',
      e3: './e3.js',
    },
    output: {
      minify: true,
    },
    experimental: {
      minChunkSize: 100_000,
    },
  },
  afterTest: async (output) => {
    const chunks = output.output.filter((c) => c.type === 'chunk');
    // Leaf duplicated into all three entries -> 3 chunks, no standalone shared chunk.
    expect(chunks.length).toBe(3);
    const chunksWithUtil = chunks.filter((c) =>
      c.moduleIds.some((id) => id.replace(/\\/g, '/').endsWith('/util.js')),
    );
    expect(chunksWithUtil.length).toBe(3);
    for (const c of chunks) {
      expect(c.imports.length).toBe(0);
    }
    const e1 = await import('./dist/e1.js');
    const e2 = await import('./dist/e2.js');
    const e3 = await import('./dist/e3.js');
    expect(e1.r1).toBe('A1');
    expect(e2.r2).toBe('B2');
    expect(e3.r3).toBe('AB3');
  },
});
