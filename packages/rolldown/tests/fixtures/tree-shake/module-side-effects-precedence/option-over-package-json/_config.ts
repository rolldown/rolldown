import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// `moduleSideEffects` precedence: `treeshake.moduleSideEffects` option > `package.json`
// `sideEffects` field
export default defineTest({
  config: {
    treeshake: {
      moduleSideEffects(id) {
        if (id.includes('dep.js')) return true;
        return undefined;
      },
    },
  },
  afterTest: (output) => {
    const code = output.output[0].code;
    expect(code).toContain('sideeffects');
  },
});
