import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// `moduleSideEffects` precedence: `resolveId` > `treeshake.moduleSideEffects` option
export default defineTest({
  config: {
    treeshake: {
      moduleSideEffects: false,
    },
    plugins: [
      {
        name: 'precedence',
        resolveId(id) {
          if (id === 'virtual:dep') return { id: '\0dep', moduleSideEffects: true };
          return null;
        },
        load(id) {
          if (id === '\0dep') {
            return 'console.log("sideeffects")';
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
