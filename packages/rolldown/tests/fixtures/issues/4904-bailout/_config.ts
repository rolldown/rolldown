import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: ['./mod1.js', './main.js'],
    preserveEntrySignatures: 'strict',
  },
  afterTest(output) {
    let o = output.output;
    // no merging optimization applied
    expect(o.length).toBe(4);
  },
});
