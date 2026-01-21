import nodePath from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect, vi } from 'vitest';

const fn = vi.fn();

export default defineTest({
  sequential: true,
  config: {
    plugins: [
      {
        name: 'test-plugin-context',
        async buildStart(this) {
          const ret = await this.resolve('./sub.js', undefined, {
            kind: 'require-call'
          });
          if (!ret) {
            throw new Error('resolve failed');
          }
        },
      },
      {
        name: 'test-plugin-kind',
        resolveId(id, _importer, options) {
          if (id === './sub.js') {
            expect(options.kind).toBe('require-call');
            fn();
            return nodePath.resolve(import.meta.dirname, 'main.js');
          }
        },
      },
    ],
  },
  afterTest: () => {
    expect(fn).toHaveBeenCalledTimes(1);
  },
});
