import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: './main.js',
  },
  afterTest: (output) => {
    const code = output.output[0].code;
    // Both imports should use __toESM since one of them is a default import
    expect(code).toContain('__toESM');
    // The import should be: var import_react = __toESM(require_react(), 1);
    // Not: var import_react = require_react();
  },
});
