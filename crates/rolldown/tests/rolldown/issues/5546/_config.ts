import { defineTest } from "rolldown-tests";
import { expect } from "vitest";

export default defineTest({
  config: {
    output: {
      format: "esm",
    },
    treeshake: true,
  },
  afterTest: (output) => {
    const chunk = output.output[0];
    const code = chunk.code;
    
    // The issue: duplicate exports.default assignments inside CommonJS wrapper
    // should be reduced/optimized when they assign the same value
    const cjsWrapperMatch = code.match(/var require_main = .*?\{.*?\}\)/s);
    if (cjsWrapperMatch) {
      const wrapperCode = cjsWrapperMatch[0];
      
      // With the fix, we should see the assignments optimized using comma operator
      // or eliminated entirely when redundant
      const defaultExportMatches = wrapperCode.match(/exports\.default\s*=/g);
      
      // The optimization should reduce redundant separate assignments
      // Either by eliminating redundant ones or combining using comma operator
      expect(defaultExportMatches ? defaultExportMatches.length : 0).toBeLessThanOrEqual(2);
      
      // Should not have separate redundant statements on different lines
      expect(wrapperCode).not.toMatch(/exports\.default\s*=\s*localeValues;\s*[\r\n]\s*.*exports\.default\s*=\s*localeValues/);
    }
  },
});