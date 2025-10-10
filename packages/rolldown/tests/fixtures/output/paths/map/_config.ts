import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    external: ['d3', 'd4'],
    output: {
      paths: {
        d3: 'https://cdn.jsdelivr.net/npm/d3@7.8.5/dist/d3.min.js'
      },
    },
  },
  afterTest: (output) => {
    expect(output.output[0].code).toMatchInlineSnapshot(`
      "import { a } from "https://cdn.jsdelivr.net/npm/d3@7.8.5/dist/d3.min.js";
      import { b } from "d4";

      //#region main.js
      console.log(a, b);

      //#endregion"
    `)

  },
})
