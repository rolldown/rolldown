import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    external: ['./dep.js'],
    output: {
      format: 'system',
      // systemNullSetters defaults to true: unused setters become `null`
      systemNullSetters: true,
    },
  },
  afterTest: (output) => {
    const chunk = output.output[0];
    if (chunk.type === 'chunk') {
      // With systemNullSetters: true, the setter for the side-effect-only
      // external dep should be `null` rather than an empty function.
      expect(chunk.code).toContain('null');
      expect(chunk.code).not.toContain('function() {}');
    }
  },
});
