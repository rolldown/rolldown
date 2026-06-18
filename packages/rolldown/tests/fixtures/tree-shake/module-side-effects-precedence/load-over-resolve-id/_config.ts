import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// `moduleSideEffects` precedence: `load` > `resolveId`
export default defineTest({
  config: {
    plugins: [
      {
        name: 'precedence',
        resolveId(id) {
          if (id === 'virtual:dep') return { id: '\0dep', moduleSideEffects: true };
          return null;
        },
        load(id) {
          if (id === '\0dep') {
            return { code: 'console.log("sideeffects")', moduleSideEffects: false };
          }
          return null;
        },
      },
    ],
  },
  afterTest: (output) => {
    const code = output.output[0].code;
    expect(code).not.toContain('sideeffects');
  },
});
