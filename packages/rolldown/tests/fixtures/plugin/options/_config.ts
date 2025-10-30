import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect, vi } from 'vitest';

const fn = vi.fn();

export default defineTest({
  config: {
    input: [],
    plugins: [
      {
        name: 'test-plugin',
        options: function(opts) {
          expect(opts.input?.length).toBe(0);
          opts.input = [path.join(__dirname, 'main.js')];
          fn();
          return opts;
        },
      },
    ],
  },
  afterTest: (output) => {
    expect(fn).toHaveBeenCalledTimes(1);
    expect(getOutputChunk(output).length).toBe(1);
  },
});
