import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// `moduleSideEffects` precedence: `transform` > `load`
export default defineTest({
  config: {
    plugins: [
      {
        name: 'precedence',
        resolveId(id) {
          if (id === 'virtual:dep') return '\0dep';
          return null;
        },
        load(id) {
          if (id === '\0dep') {
            return { code: 'console.log("sideeffects")', moduleSideEffects: true };
          }
          return null;
        },
        transform(code, id) {
          if (id === '\0dep') {
            return { code, moduleSideEffects: false };
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
