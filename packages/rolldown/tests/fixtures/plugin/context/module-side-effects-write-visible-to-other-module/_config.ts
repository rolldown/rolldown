import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect, vi } from 'vitest';

const main = path.join(import.meta.dirname, 'main.js');
const side = path.join(import.meta.dirname, 'side.js');
const dep = path.join(import.meta.dirname, 'dep.js');
const fn = vi.fn();

// A write in a hook of one module is visible at once in a hook of another module, and it reaches
// tree shaking.
export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        async transform(_code, id) {
          if (id !== main) return;
          // `side.js` is parsed when `this.load` returns, so no hook of `side.js` runs later.
          await this.load({ id: side });
          this.getModuleInfo(side)!.moduleSideEffects = false;
        },
        load(id) {
          // `dep.js` is fetched only after `main.js` is transformed.
          if (id !== dep) return;
          expect(this.getModuleInfo(side)!.moduleSideEffects).toBe(false);
          fn();
        },
      },
    ],
  },
  afterTest(output) {
    expect(fn).toHaveBeenCalledTimes(1);
    const [chunk] = getOutputChunk(output);
    expect(chunk.code).toContain('main');
    expect(chunk.code).toContain('dep');
    expect(chunk.code).not.toContain('SIDE_EFFECT');
  },
});
