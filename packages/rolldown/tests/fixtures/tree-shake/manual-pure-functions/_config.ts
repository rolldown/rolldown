import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    treeshake: {
      manualPureFunctions: ['styled', 'local'],
    },
    external: ['styled-components'],
  },
  afterTest: (output) => {
    let code = output.output[0].code;

    expect(code).toMatchInlineSnapshot(`
      "import styled from "styled-components";
      //#region main.js
      function effect(value) {
      \tconsole.log(value);
      \treturn value;
      }
      styled()[effect("computed key")];
      styled(effect("call argument")).value;
      new (styled())[effect("new callee")].Box();
      let another = console.log;
      another();
      //#endregion
      "
    `);
  },
});
