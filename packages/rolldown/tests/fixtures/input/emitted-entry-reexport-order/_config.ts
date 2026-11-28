import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: { early: './early.js', later: './later.js' },
    plugins: [
      {
        name: 'emit-lib',
        buildStart() {
          this.emitFile({ type: 'chunk', id: './lib/index.js', name: 'index' });
        },
      },
    ],
  },
  afterTest: async () => {
    const { result } = await import('./dist/later.js' as string);
    expect(result).toEqual(['ready', 'dummy']);
  },
});
