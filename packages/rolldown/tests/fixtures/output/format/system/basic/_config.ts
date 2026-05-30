import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    output: {
      format: 'system',
    },
  },
  afterTest: (output) => {
    const chunk = output.output[0];
    expect(chunk.type).toBe('chunk');
    if (chunk.type === 'chunk') {
      expect(chunk.code).toMatchInlineSnapshot(`
        "System.register([], (function(exports) {
        	"use strict";
        	return { execute: (function() {
        		exports("add", (a, b) => a + b);
        		exports("PI", 3.14);
        		//#endregion
        	}) };
        }));
        "
      `);
    }
  },
});
