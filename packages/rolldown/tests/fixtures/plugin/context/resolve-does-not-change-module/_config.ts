import path from 'node:path';
import type { PluginContext } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect, vi } from 'vitest';

const main = path.join(import.meta.dirname, 'main.js');
const b = path.join(import.meta.dirname, 'b.js');
const fn = vi.fn();

async function probe(ctx: PluginContext) {
  const resolved = await ctx.resolve('./b.js', main, { custom: { probe: true } });
  expect(resolved!.meta).toStrictEqual({ probe: true });
  expect(resolved!.moduleSideEffects).toBe(false);
  // The result is not the module's meta object.
  resolved!.meta.changedByProbe = true;
}

export default defineTest({
  config: {
    plugins: [
      {
        name: 'resolver',
        resolveId(source, _importer, options) {
          if (source !== './b.js') return;
          return options.custom?.probe
            ? { id: b, meta: { probe: true }, moduleSideEffects: false }
            : { id: b, meta: { normal: true } };
        },
      },
      {
        // `this.resolve` skips the plugin that calls it, so the probes come from a second plugin.
        name: 'prober',
        async transform(_code, id) {
          // `main.js` is transformed before its import creates `b.js`.
          if (id === main) await probe(this);
        },
        async buildEnd() {
          await probe(this);
          const info = this.getModuleInfo(b)!;
          expect(info.meta).toStrictEqual({ normal: true });
          expect(info.moduleSideEffects).toBe(true);
          fn();
        },
      },
    ],
  },
  afterTest(output) {
    expect(fn).toHaveBeenCalledTimes(1);
    expect(getOutputChunk(output)[0].code).toContain('B_EFFECT');
  },
});
