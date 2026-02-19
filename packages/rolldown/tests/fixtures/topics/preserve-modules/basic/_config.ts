import type { OutputChunk } from 'rolldown';
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
    expect(output.output[0].code).toMatchInlineSnapshot(`
      "import { lib } from "./lib.js";
      import { a } from "./src/index.js";

      //#region main.js
      console.log(lib, a);

      //#endregion"
    `);
    expect(output.output[1].fileName).toMatchInlineSnapshot(`"lib.js"`);
    expect((output.output[1] as OutputChunk).code).toMatchInlineSnapshot(`
      "//#region lib.js
      const lib = "lib";

      //#endregion
      export { lib };"
    `);
    expect(output.output[2].fileName).toMatchInlineSnapshot(`"src/index.js"`);
    expect((output.output[2] as OutputChunk).code).toMatchInlineSnapshot(`
      "import "./module.js";
      
      //#region src/index.js
      const a = 100;

      //#endregion
      export { a };"
    `);
    expect(output.output[4].fileName).toMatch(/^src\/module-[a-zA-Z0-9]+\.css$/);
  },
});
