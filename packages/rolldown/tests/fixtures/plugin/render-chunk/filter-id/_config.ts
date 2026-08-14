import { id, include } from '@rolldown/pluginutils';
import { defineTest } from 'rolldown-tests';
import { expect, vi } from 'vitest';

const renderChunkFn = vi.fn();

// `renderChunk` has no `id` to test, but its `filter` is typed as an arbitrary
// `TopLevelFilterExpression[]`, so an `id` leaf is reachable here. It must report
// "no match" rather than crashing the build -- this fixture panics on a rolldown
// without the fix, and pins the hook-level outcome the Rust unit tests can't reach.
export default defineTest({
  config: {
    input: './main.js',
    plugins: [
      {
        name: 'testIdFilterOnRenderChunk',
        renderChunk: {
          filter: [include(id(/main\.js$/))],
          handler(_) {
            renderChunkFn();
            return null;
          },
        },
      },
    ],
  },
  afterTest: () => {
    expect(renderChunkFn).toHaveBeenCalledTimes(0);
  },
});
