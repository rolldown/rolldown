import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// Per-chunk `renderChunk` invocations run concurrently (the Rust side drives
// them through `try_join_all`), so a hook that reads `meta.magicString` after
// an `await` must still see its OWN chunk -- not the chunk whose invocation
// started last, nor a box that invocation's cleanup already released on the
// threadless flavor. Chunk a's hook yields until chunk b's invocation has
// fully settled before its first read to pin exactly that.
const observed: Record<string, string> = {};
let resolveBDone!: () => void;
const bDone = new Promise<void>((resolve) => {
  resolveBDone = resolve;
});

export default defineTest({
  // Module-level state shared between the hooks and `afterTest`.
  sequential: true,
  config: {
    input: ['a.js', 'b.js'],
    experimental: {
      nativeMagicString: true,
    },
    plugins: [
      {
        name: 'test-render-chunk-interleaved-magic-string',
        async renderChunk(_code, chunk, _options, meta) {
          if (chunk.fileName.startsWith('a')) {
            await Promise.race([
              bDone,
              new Promise((_, reject) =>
                setTimeout(
                  () => reject(new Error('renderChunk invocations did not interleave')),
                  10_000,
                ),
              ),
            ]);
            // One macrotask more, so b's wrapper cleanup has run too.
            await new Promise((resolve) => setTimeout(resolve, 20));
          }
          observed[chunk.fileName] = meta.magicString!.original;
          if (chunk.fileName.startsWith('b')) {
            resolveBDone();
          }
          return null;
        },
      },
    ],
  },
  afterTest() {
    expect(observed['a.js']).toContain('chunk-a-marker');
    expect(observed['a.js']).not.toContain('chunk-b-marker');
    expect(observed['b.js']).toContain('chunk-b-marker');
  },
});
