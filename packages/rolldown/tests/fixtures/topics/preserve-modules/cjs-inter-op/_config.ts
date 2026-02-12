import { OutputChunk } from 'rolldown';
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
    expect(output.output[1].fileName.replace(/\\/g, '/')).toMatchInlineSnapshot(
      `"_virtual/_rolldown/runtime.js"`,
    );

    expect(output.output[2].fileName).toMatchInlineSnapshot(`"lib.js"`);
    expect((output.output[2] as OutputChunk).code).toMatchInlineSnapshot(`
      "import { __commonJSMin } from "./_virtual/_rolldown/runtime.js";

      //#region lib.js
      var require_lib = /* @__PURE__ */ __commonJSMin(((exports, module) => {
      	module.exports = 1e3;
      }));

      //#endregion
      export default require_lib();

      export { require_lib };"
    `);

    expect(output.output[3].fileName.replace(/\\/g, '/')).toMatchInlineSnapshot(`"src/index.js"`);
  },
});
