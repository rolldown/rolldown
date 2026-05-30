import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    output: {
      format: 'system',
      // name causes System.register("my-lib", ...) — named registration
      name: 'my-lib',
    },
  },
  afterTest: (output) => {
    const chunk = output.output[0];
    if (chunk.type === 'chunk') {
      expect(chunk.code).toContain('System.register("my-lib",');
    }
  },
});
