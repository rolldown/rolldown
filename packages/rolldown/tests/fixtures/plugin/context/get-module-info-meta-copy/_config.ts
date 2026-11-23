import fs from 'node:fs';
import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect, vi } from 'vitest';

const p = path.join(import.meta.dirname, 'p.js');
const q = path.join(import.meta.dirname, 'q.js');
// One object that `resolveId` returns as the meta of both modules.
const resolvedMeta = { resolved: true };
const fn = vi.fn();

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        resolveId(source) {
          if (source === './p.js') return { id: p, meta: resolvedMeta };
          if (source === './q.js') return { id: q, meta: resolvedMeta };
        },
        load(id) {
          if (id === p) return { code: fs.readFileSync(id, 'utf8'), meta: { loadedP: true } };
          if (id === q) return { code: fs.readFileSync(id, 'utf8'), meta: { loadedQ: true } };
        },
        buildEnd() {
          const pMeta = this.getModuleInfo(p)!.meta;
          const qMeta = this.getModuleInfo(q)!.meta;
          expect(pMeta).toStrictEqual({ resolved: true, loadedP: true });
          expect(qMeta).toStrictEqual({ resolved: true, loadedQ: true });
          expect(pMeta).not.toBe(resolvedMeta);
          expect(pMeta).not.toBe(qMeta);
          expect(resolvedMeta).toStrictEqual({ resolved: true });
          fn();
        },
      },
    ],
  },
  afterTest() {
    expect(fn).toHaveBeenCalledTimes(1);
  },
});
