import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect, vi } from 'vitest';

const fn = vi.fn();
const dep = path.join(import.meta.dirname, 'dep.js');

export default defineTest({
  sequential: true,
  config: {
    plugins: [
      {
        name: 'test-plugin-context',
        resolveId(source) {
          if (source === './dep.js') {
            return { id: dep, meta: { resolved: true } };
          }
        },
        transform(code, id) {
          if (id === dep) {
            return { code, moduleSideEffects: false, meta: { transformed: true } };
          }
        },
        async buildEnd() {
          const ret = await this.resolve('./dep.js', undefined, { skipSelf: false });
          expect(ret?.meta).toEqual({ resolved: true });
          expect(ret?.moduleSideEffects).toBe(null);
          fn();
        },
      },
    ],
  },
  afterTest: () => {
    expect(fn).toHaveBeenCalledTimes(1);
  },
});
