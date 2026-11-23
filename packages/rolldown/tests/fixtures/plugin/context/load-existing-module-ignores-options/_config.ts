import fs from 'node:fs';
import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect, vi } from 'vitest';

const dep = path.join(import.meta.dirname, 'dep.js');
const late = path.join(import.meta.dirname, 'late.js');
let markDepLoadStarted!: () => void;
const depLoadStarted = new Promise<void>((resolve) => {
  markDepLoadStarted = resolve;
});
let markLoadCalled!: () => void;
const loadCalled = new Promise<void>((resolve) => {
  markLoadCalled = resolve;
});
const fn = vi.fn();

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        resolveId(source) {
          if (source === './dep.js') return { id: dep, meta: { fromResolve: true } };
        },
        async load(id) {
          if (id !== dep) return;
          markDepLoadStarted();
          // Keep `dep.js` unparsed until `this.load` was called for it.
          await loadCalled;
          return { code: fs.readFileSync(dep, 'utf8') };
        },
        buildEnd() {
          const info = this.getModuleInfo(dep)!;
          expect(info.meta).toStrictEqual({ fromResolve: true });
          expect(info.moduleSideEffects).toBe(true);
          fn();
        },
      },
      {
        name: 'loader',
        async transform(_code, id) {
          if (id !== late) return;
          await depLoadStarted;
          const loaded = this.load({ id: dep, meta: { fromLoad: true }, moduleSideEffects: false });
          markLoadCalled();
          expect((await loaded).meta).toStrictEqual({ fromResolve: true });
        },
      },
    ],
  },
  afterTest(output) {
    expect(fn).toHaveBeenCalledTimes(1);
    expect(getOutputChunk(output)[0].code).toContain('DEP_EFFECT');
  },
});
