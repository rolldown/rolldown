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
    
    // Verify that both module-a and module-b imports use __toESM
    // module-a.js should have: var import_react$1 = __toESM(require_react(), 1);
    // module-b.js should have: var import_react = __toESM(require_react(), 1);
    const toEsmMatches = code.match(/__toESM\(require_react\(\), 1\)/g);
    expect(toEsmMatches).toBeTruthy();
    expect(toEsmMatches?.length).toBe(2); // Both imports should use __toESM
    
    // Verify that neither import uses bare require_react() without __toESM
    // (except for the __commonJS definition itself)
    const bareRequireMatches = code.match(/= require_react\(\);/g);
    expect(bareRequireMatches).toBeNull(); // Should not have any bare require_react() assignments
  },
});
