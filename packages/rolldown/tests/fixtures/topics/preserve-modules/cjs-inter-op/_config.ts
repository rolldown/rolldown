import { expect } from "vitest";
import { defineTest } from "rolldown-tests";
import { OutputChunk } from "rolldown";

export default defineTest({
  config: {
    output: {
      preserveModules: true,
    },
  },
  afterTest: (output) => {
    expect(output.output[0].fileName).toMatchInlineSnapshot(`"main.js"`);
    expect(output.output[1].fileName.replace(/\\/g, '/')).toMatchInlineSnapshot(
      `"_virtual/rolldown_runtime.js"`,
    );

    expect(output.output[2].fileName).toMatchInlineSnapshot(`"lib.js"`);
    expect((output.output[2] as OutputChunk).code).toMatchInlineSnapshot(`
      "import { __commonJS } from "./_virtual/rolldown_runtime.js";

      //#region lib.js
      var require_lib = /* @__PURE__ */ __commonJS({ "lib.js": ((exports, module) => {
      	module.exports = 1e3;
      }) });

      //#endregion
      export default require_lib();

      export { require_lib };"
    `);

    expect(output.output[3].fileName.replace(/\\/g, '/')).toMatchInlineSnapshot(`"src/index.js"`);
  },
});
