import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: ['./main.ts'],
    tsconfig: './tsconfig.json',
    output: {
      format: 'esm',
    },
  },
  async afterTest(output) {
    const chunk = output.output[0];
    expect(chunk.type).toBe('chunk');
    if (chunk.type === 'chunk') {
      // Should NOT inject "use strict" for ESM format (already strict)
      expect(chunk.code).not.toContain('"use strict"');
    }
  },
});
