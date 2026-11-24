import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect, vi } from 'vitest';

const main = path.join(import.meta.dirname, 'main.js');
const a = path.join(import.meta.dirname, 'a.js');
const fn = vi.fn();

export default defineTest({
  config: {
    plugins: [
      {
        name: 'resolver',
        resolveId(source, _importer, options) {
          if (source !== './a.js') return;
          if (options.custom?.answer === 'own') return { id: a, meta: { probe: true } };
          // Let the default resolver decide.
          if (options.custom?.answer === 'none') return;
          return { id: a, meta: { resolved: true }, moduleSideEffects: false };
        },
        transform(code, id) {
          if (id === a) return { code, meta: { transformed: true } };
        },
      },
      {
        name: 'prober',
        async buildEnd() {
          const moduleMeta = this.getModuleInfo(a)!.meta;
          expect(moduleMeta).toStrictEqual({ resolved: true, transformed: true });

          const own = await this.resolve('./a.js', main, { custom: { answer: 'own' } });
          expect(own!.meta).toStrictEqual({ probe: true });
          // No hook decided, so it reports the value from the option, `package.json` or `true`.
          expect(own!.moduleSideEffects).toBe(true);
          expect(own).not.toHaveProperty('invalidate');
          expect(own).not.toHaveProperty('moduleOptionsReceipt');

          const none = await this.resolve('./a.js', main, { custom: { answer: 'none' } });
          expect(none!.id).toBe(a);
          expect(none!.meta).toStrictEqual({});
          expect(none!.meta).not.toBe(moduleMeta);
          expect(none!.moduleSideEffects).toBe(true);
          fn();
        },
      },
    ],
  },
  afterTest() {
    expect(fn).toHaveBeenCalledTimes(1);
  },
});
