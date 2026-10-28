import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const logs: { code?: string }[] = [];

// https://github.com/rolldown/rolldown/issues/11058: `a.js` and `b.js` both re-export the same
// binding of `ext`, so `InjectionToken` is not ambiguous and `main.js` must export it.
export default defineTest({
  config: {
    external: ['ext'],
    output: [
      { format: 'esm', entryFileNames: 'main.mjs' },
      { format: 'cjs', entryFileNames: 'main.cjs' },
    ],
    onLog(_level, log) {
      logs.push(log);
    },
  },
  afterTest: (outputs) => {
    for (const output of outputs) {
      expect(output.output[0].exports).toStrictEqual(['InjectionToken', 'Injector']);
    }
    expect(logs).toStrictEqual([]);
  },
});
