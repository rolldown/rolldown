import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    treeshake: {
      manualPureFunctions: ['styled'],
    },
    external: ['styled-components'],
  },
  afterTest: (output) => {
    let code = output.output[0].code;

    expect(code).toMatchInlineSnapshot(`
      "import "styled-components";
      "
    `);
  },
});
