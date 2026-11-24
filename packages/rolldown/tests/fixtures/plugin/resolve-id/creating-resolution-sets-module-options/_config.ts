import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect, vi } from 'vitest';

const b = path.join(import.meta.dirname, 'b.js');
const x = path.join(import.meta.dirname, 'x.js');
let markXCreated!: () => void;
const xCreated = new Promise<void>((resolve) => {
  markXCreated = resolve;
});
const fn = vi.fn();

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        async resolveId(source, importer) {
          if (source !== './x.js') return;
          if (importer === b) {
            return { id: x, meta: { fromB: true }, moduleSideEffects: false };
          }
          // Resolve the import in `c.js` only after the import in `b.js` created `x.js`.
          await xCreated;
          return { id: x, meta: { fromC: true }, moduleSideEffects: true };
        },
        load(id) {
          if (id === x) markXCreated();
        },
        transform(code, id) {
          if (id === x) return { code };
        },
        buildEnd() {
          const info = this.getModuleInfo(x)!;
          expect(info.meta).toStrictEqual({ fromB: true });
          expect(info.moduleSideEffects).toBe(false);
          fn();
        },
      },
    ],
  },
  afterTest(output) {
    expect(fn).toHaveBeenCalledTimes(1);
    expect(getOutputChunk(output)[0].code).not.toContain('X_EFFECT');
  },
});
