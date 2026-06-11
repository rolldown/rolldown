import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const packageJsonPath = path.join(import.meta.dirname, 'package.json');

// An explicit `moduleSideEffects` from a hook must take priority over the
// `package.json#sideEffects`, so the inline module's side effect must be kept.
export default defineTest({
  config: {
    plugins: [
      {
        name: 'inline-side-effects-repro',
        resolveId(id) {
          if (id === 'virtual:dep') return { id: '\0dep', packageJsonPath };
          return null;
        },
        load(id) {
          if (id === '\0dep') {
            return { code: 'console.log("sideeffects")', moduleSideEffects: true };
          }
          return null;
        },
      },
    ],
  },
  afterTest: (output) => {
    const code = output.output[0].code;
    expect(code).toContain('sideeffects');
  },
});
