import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'
import {OutputChunk} from 'rolldown'


export default defineTest({
  config: {
    output: {
      preserveModules: true
    }
  },
  afterTest: (output) => {
    expect(output.output[0].fileName).toMatchInlineSnapshot(`"main.js"`);
    expect(output.output[0].code).toMatchInlineSnapshot(`
      "import { a } from "./src/index-DGfmz2Yl.js";
      import { lib } from "./lib-i3NB89bx.js";

      //#region main.js
      console.log(lib, a);

      //#endregion"
    `);
    expect(output.output[1].fileName).toMatchInlineSnapshot(`"lib-i3NB89bx.js"`);
    expect((output.output[1] as OutputChunk).code).toMatchInlineSnapshot(`
      "//#region lib.js
      const lib = "lib";

      //#endregion
      export { lib };"
    `)
    expect(output.output[2].fileName).toMatchInlineSnapshot(`"src/index-DGfmz2Yl.js"`);
    expect((output.output[2] as OutputChunk).code).toMatchInlineSnapshot(`
      "//#region src/index.js
      const a = 100;

      //#endregion
      export { a };"
    `)
  },
})
