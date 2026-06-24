import { expect } from 'vitest';
import { defineTest } from 'rolldown-tests';

// Conservative-eligibility guard: `util.js`'s `k` is re-exported by
// `re-exporter.js`, and entry `b` imports it through that re-export chain. The
// direct-import-edge importer scan can't see `b` as an importer of `util.js`, so
// duplicating the leaf would drop `k` from `b`'s chunk. The pass therefore
// EXCLUDES re-exported leaves: `util.js` must stay a single shared chunk (never
// duplicated), and both entries must still run correctly.
export default defineTest({
  config: {
    input: {
      a: './a.js',
      b: './b.js',
    },
    experimental: {
      minChunkSize: 100_000,
    },
  },
  afterTest: async (output) => {
    const chunks = output.output.filter((c) => c.type === 'chunk');
    const chunksWithUtil = chunks.filter((c) =>
      c.moduleIds.some((id) => id.replace(/\\/g, '/').endsWith('/util.js')),
    );
    // util.js was NOT duplicated (it is re-exported) -> present in exactly one chunk.
    expect(chunksWithUtil.length).toBe(1);
    // Runtime stays correct for both the direct importer and the re-export chain.
    const a = await import('./dist/a.js');
    const b = await import('./dist/b.js');
    expect(a.a).toBe('Ka');
    expect(b.b).toBe('Kb');
  },
});
