import { defineTest } from 'rolldown-tests';
import { expect, vi } from 'vitest';

const resolveIdFn = vi.fn();

export default defineTest({
  config: {
    input: './main.js',
    plugins: [
      {
        name: 'test-plugin',
        resolveId(id) {
          if (id === './foo') {
            resolveIdFn();
          }
        },
      },
    ],
  },
  afterTest: () => {
    // './foo' is imported 3 times but resolve_id should only be called once
    expect(resolveIdFn).toHaveBeenCalledTimes(1);
  },
});
