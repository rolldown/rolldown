import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: ['./main.ts'],
    tsconfig: false,
    output: {
      format: 'cjs',
    },
  },
  async afterTest(output) {
    const chunk = output.output[0];
    expect(chunk.type).toBe('chunk');
    if (chunk.type === 'chunk') {
      // Should NOT inject "use strict" when tsconfig is disabled
      expect(chunk.code).not.toContain('"use strict"');
    }
  },
});
