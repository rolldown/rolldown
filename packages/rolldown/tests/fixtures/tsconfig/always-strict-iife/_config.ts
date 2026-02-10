import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: ['./main.ts'],
    tsconfig: './tsconfig.json',
    output: {
      format: 'iife',
      name: 'MyBundle',
    },
  },
  async afterTest(output) {
    const chunk = output.output[0];
    expect(chunk.type).toBe('chunk');
    if (chunk.type === 'chunk') {
      // Should inject "use strict" for IIFE format when alwaysStrict is true
      expect(chunk.code).toContain('"use strict"');
    }
  },
});
