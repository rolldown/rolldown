import { defineTest } from 'rolldown-tests';
import { expect, vi } from 'vitest';

const transformFn = vi.fn();

export default defineTest({
  config: {
    input: {
      main: './main.js',
      foo: './foo.txt',
    },
    plugins: [
      {
        name: 'test-plugin',
        buildStart(this) {
          // Set filter to only process .txt files after buildStart is called
          this.setHookFilter({
            transform: {
              id: { include: /\.txt$/ },
            },
          });
        },
        transform(code, id) {
          transformFn(id);
          return code;
        },
      },
    ],
  },
  afterTest: (_output) => {
    // Transform should only be called for foo.txt, not main.js
    expect(transformFn).toHaveBeenCalledTimes(1);
    expect(transformFn).toHaveBeenCalledWith(
      expect.stringContaining('foo.txt'),
    );
  },
});
