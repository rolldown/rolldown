import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// Two plugins calling `this.resolve` on each other with the same specifier used to recurse
// forever. The call below must settle with the first plugin falling back to its own result.
const calls: string[] = [];

export default defineTest({
  sequential: true,
  config: {
    input: './main.js',
    plugins: [
      {
        name: 'r1',
        async resolveId(id) {
          if (id !== 'entry-x') return null;
          calls.push('r1');
          // Guard against runaway recursion so a regression fails instead of hanging.
          if (calls.length > 20) throw new Error('resolve recursion did not terminate');
          return (await this.resolve(id, 'foo')) ?? 'success';
        },
        load(id) {
          if (id === 'success') {
            return { code: 'export default 1' };
          }
        },
      },
      {
        name: 'r2',
        resolveId(id) {
          if (id !== 'entry-x') return null;
          calls.push('r2');
          return this.resolve(id, 'bar');
        },
      },
    ],
  },
  afterTest: () => {
    expect(calls.length).toBeLessThanOrEqual(20);
    expect(calls).toEqual(['r1', 'r2', 'r1']);
  },
});
