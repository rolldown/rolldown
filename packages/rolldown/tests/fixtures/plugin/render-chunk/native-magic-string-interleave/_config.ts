import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// Per-chunk `renderChunk` calls run concurrently, so a hook reading
// `meta.magicString` after an `await` must see its OWN chunk, not the chunk
// whose call started last. Chunk a's hook waits until chunk b's call has
// settled before reading.
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
            // One macrotask more, so b's call has fully returned.
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
