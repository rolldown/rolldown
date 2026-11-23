import path from 'node:path';
import type { ModuleInfo } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect, vi } from 'vitest';

const dep = path.join(import.meta.dirname, 'dep.js');
const fn = vi.fn();
let infoFromLoad: ModuleInfo | undefined;

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        load(id) {
          if (id !== dep) return;
          infoFromLoad = this.getModuleInfo(id)!;
          expect(infoFromLoad.moduleSideEffects).toBe(true);
        },
        moduleParsed(info) {
          if (info.id !== dep) return;
          // The argument has the same setter as `getModuleInfo`.
          info.moduleSideEffects = false;
        },
        buildEnd() {
          // An info object read before the write shows the new value.
          expect(infoFromLoad!.moduleSideEffects).toBe(false);
          const info = this.getModuleInfo(dep)!;
          expect(info.moduleSideEffects).toBe(false);
          expect('invalidate' in info).toBe(false);
          expect(Object.keys(info)).toContain('moduleSideEffects');
          fn();
        },
      },
    ],
  },
  afterTest(output) {
    expect(fn).toHaveBeenCalledTimes(1);
    const [chunk] = getOutputChunk(output);
    expect(chunk.code).toContain('main');
    expect(chunk.code).not.toContain('DEP_EFFECT');
  },
});
