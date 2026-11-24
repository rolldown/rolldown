import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect, vi } from 'vitest';

const b = path.join(import.meta.dirname, 'b.js');
const fn = vi.fn();

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        resolveId(source, importer) {
          if (source !== 'ext') return;
          return importer === b
            ? { id: 'ext', external: true, meta: { fromB: true }, moduleSideEffects: false }
            : { id: 'ext', external: true, meta: { fromC: true }, moduleSideEffects: true };
        },
        buildEnd() {
          const info = this.getModuleInfo('ext')!;
          // Either import may create the external, but only that one sets its options.
          expect([
            { meta: { fromB: true }, moduleSideEffects: false },
            { meta: { fromC: true }, moduleSideEffects: true },
          ]).toContainEqual({ meta: info.meta, moduleSideEffects: info.moduleSideEffects });
          fn();
        },
      },
    ],
  },
  afterTest() {
    expect(fn).toHaveBeenCalledTimes(1);
  },
});
