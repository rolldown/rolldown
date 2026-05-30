import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    output: {
      format: 'system',
    },
  },
  afterTest: (output) => {
    const chunk = output.output[0];
    if (chunk.type === 'chunk') {
      // Top-level await requires the execute function to be async
      expect(chunk.code).toContain('async function');
      expect(chunk.code).toContain('await Promise.resolve(42)');
    }
  },
});
