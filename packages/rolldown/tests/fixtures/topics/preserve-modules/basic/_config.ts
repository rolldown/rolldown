import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    output: {
      preserveModules: true,
    },
  },
  afterTest: (output) => {
    expect(output.output[0].fileName).toMatchInlineSnapshot(`"main.js"`);
    expect(output.output[1].fileName).toMatchInlineSnapshot(`"lib.js"`);
    expect(output.output[2].fileName).toMatchInlineSnapshot(`"src/index.js"`);
  },
});
