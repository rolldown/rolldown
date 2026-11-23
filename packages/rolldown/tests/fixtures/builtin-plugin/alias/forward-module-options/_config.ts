import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { viteAliasPlugin } from 'rolldown/experimental';
import { expect, vi } from 'vitest';

const dep = path.join(import.meta.dirname, 'dep.js');
const fn = vi.fn();

export default defineTest({
  config: {
    plugins: [
      // The alias plugin resolves `./dep.js` with the hook below and returns that resolution.
      viteAliasPlugin({ entries: [{ find: '@dep', replacement: './dep.js' }] }),
      {
        name: 'test-plugin',
        resolveId(source) {
          if (source === './dep.js') {
            return { id: dep, meta: { fromHook: true }, moduleSideEffects: false };
          }
        },
        buildEnd() {
          const info = this.getModuleInfo(dep)!;
          expect(info.meta).toStrictEqual({ fromHook: true });
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
