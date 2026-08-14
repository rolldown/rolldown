import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: ['./mod1.js', './main.js'],
    preserveEntrySignatures: 'strict',
  },
  afterTest(output) {
    let o = output.output;
    // Entry modules stay in their entry chunk, so dynamic import of mod1 reuses the entry chunk
    expect(o.length).toBe(3);
  },
});
