import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: ['./main.ts'],
    tsconfig: './tsconfig.json',
    output: {
      format: 'cjs',
    },
  },
  async afterTest(output) {
    const chunk = output.output[0];
    expect(chunk.type).toBe('chunk');
    if (chunk.type === 'chunk') {
      // Should inject "use strict" because alwaysStrict is true in tsconfig
      expect(chunk.code).toContain('"use strict"');
      // Make sure it's at the top (after banner/hashbang if any)
      const lines = chunk.code.trim().split('\n');
      const firstNonEmptyLine = lines.find(line => line.trim().length > 0);
      expect(firstNonEmptyLine).toContain('"use strict"');
    }
  },
});
