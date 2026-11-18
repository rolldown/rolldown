import { defineTest } from 'rolldown-tests';
import { expect, vi } from 'vitest';

// `this.resolve` passes `custom` to other plugins' `resolveId` hooks through a numeric receipt.
// The receipt is looked up when the callee hook is invoked, so a receipt that is reused while an
// earlier `this.resolve` call is still in flight hands that call another call's `custom`.

function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

const seen = new Map<string, unknown>();
const fn = vi.fn();
const gates = { b: deferred(), c: deferred() };

export default defineTest({
  sequential: true,
  config: {
    plugins: [
      {
        name: 'caller',
        async buildStart() {
          const a = this.resolve('a', undefined, { custom: { test: { tag: 'A' } } });
          const b = this.resolve('b', undefined, { custom: { test: { tag: 'B' } } });
          // `a` finishes while `b` is still waiting in the `gate` plugin, so its receipt is freed.
          await a;
          // `c` is saved while `b` is still in flight. It must not get `b`'s receipt.
          const c = this.resolve('c', undefined, { custom: { test: { tag: 'C' } } });
          // `b` reaches the `recorder` plugin only now, after `c` was saved.
          gates.b.resolve();
          await b;
          gates.c.resolve();
          await c;

          expect(Object.fromEntries(seen)).toStrictEqual({ a: 'A', b: 'B', c: 'C' });
          fn();
        },
      },
      {
        name: 'gate',
        async resolveId(id) {
          if (id === 'b' || id === 'c') {
            await gates[id].promise;
          }
          return null;
        },
      },
      {
        name: 'recorder',
        resolveId(id, _importer, options) {
          if (id === 'a' || id === 'b' || id === 'c') {
            seen.set(id, (options.custom?.test as { tag?: string } | undefined)?.tag);
            return { id, external: true };
          }
          return null;
        },
      },
    ],
  },
  afterTest: () => {
    expect(fn).toHaveBeenCalledTimes(1);
  },
});
