import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    plugins: (() => {
      let releaseB!: () => void;
      let releaseC!: () => void;
      const waitB = new Promise<void>((resolve) => {
        releaseB = resolve;
      });
      const waitC = new Promise<void>((resolve) => {
        releaseC = resolve;
      });
      const received: Record<string, unknown> = {};

      return [
        {
          name: 'delay-resolve',
          resolveId: {
            order: 'pre',
            async handler(id) {
              if (id === 'B') await waitB;
              if (id === 'C') await waitC;
            },
          },
        },
        {
          name: 'test-resolve-custom',
          async buildStart() {
            const a = this.resolve('A', undefined, { custom: { tag: 'A' } });
            const b = this.resolve('B', undefined, { custom: { tag: 'B' } });
            await a;
            // A's receipt is removed while B's is still in flight.
            const c = this.resolve('C', undefined, { custom: { tag: 'C' } });
            releaseB();
            await b;
            releaseC();
            await c;
            expect(received).toEqual({ A: 'A', B: 'B', C: 'C' });
          },
        },
        {
          name: 'record-resolve-custom',
          resolveId(id, _importer, { custom }) {
            if (id === 'A' || id === 'B' || id === 'C') {
              received[id] = custom?.tag;
              return { id, external: true };
            }
          },
        },
      ];
    })(),
  },
});
