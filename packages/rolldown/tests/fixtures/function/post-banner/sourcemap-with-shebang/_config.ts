import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    output: {
      postBanner: '/* banner */',
      sourcemap: true,
    },
  },
  afterTest: (output) => {
    const chunk = output.output[0];
    if (chunk.type !== 'chunk') return;

    // The shebang should be on the first line
    const lines = chunk.code.split('\n');
    expect(lines[0]).toBe('#!/usr/bin/env node');
    // postBanner should be on the second line
    expect(lines[1]).toBe('/* banner */');

    // The sourcemap should be defined
    const map = chunk.map;
    expect(map).toBeDefined();

    // Verify the sourcemap mappings are correct.
    // With the fix, generated lines in the sourcemap correctly reflect
    // positions in the output after the shebang is moved to the front.
    // `console.log` is on line 3 (0: shebang, 1: banner, 2: region comment, 3: code)
    expect(map!.mappings).toMatchInlineSnapshot(`";;;AACA,QAAQ,IAAI,QAAQ"`);
  },
});
