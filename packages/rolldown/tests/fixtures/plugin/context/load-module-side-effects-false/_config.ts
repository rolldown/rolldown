import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect, vi } from 'vitest';

const main = path.join(import.meta.dirname, 'main.js');
const dep = path.join(import.meta.dirname, 'dep.js');
const fn = vi.fn();

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        async transform(_code, id) {
          // `main.js` is transformed before its import creates `dep.js`, so this call creates it.
          if (id === main) {
            await this.load({ id: dep, meta: { fromLoad: true }, moduleSideEffects: false });
          }
        },
        buildEnd() {
          const info = this.getModuleInfo(dep)!;
          expect(info.meta).toStrictEqual({ fromLoad: true });
          expect(info.moduleSideEffects).toBe(false);
          fn();
        },
      },
    ],
  },
  afterTest(output) {
    expect(fn).toHaveBeenCalledTimes(1);
    expect(getOutputChunk(output)[0].code).not.toContain('DEP_EFFECT');
  },
});
