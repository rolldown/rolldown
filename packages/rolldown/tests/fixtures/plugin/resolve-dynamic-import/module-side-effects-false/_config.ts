import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect, vi } from 'vitest';

const dyn = path.join(import.meta.dirname, 'dyn.js');
const fn = vi.fn();

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        resolveDynamicImport(source) {
          if (source === './dyn.js') {
            return { id: dyn, meta: { dynamic: true }, moduleSideEffects: false };
          }
        },
        buildEnd() {
          const info = this.getModuleInfo(dyn)!;
          expect(info.meta).toStrictEqual({ dynamic: true });
          expect(info.moduleSideEffects).toBe(false);
          fn();
        },
      },
    ],
  },
  afterTest() {
    expect(fn).toHaveBeenCalledTimes(1);
  },
});
