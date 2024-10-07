import { defineTest } from '@tests'
import { expect } from 'vitest'
let isComposingJs = false
export default defineTest({
  beforeTest(testKind) {
    isComposingJs = testKind === 'compose-js-plugin'
  },
  config: {
    output: {
      exports: 'named',
      format: 'iife',
      name: 'module',
      extend: true,
    },
  },
  afterTest: (output) => {
    isComposingJs
      ? expect(output.output[0].code).toMatchInlineSnapshot(`
        "(function(exports) {

        "use strict";

        //#region main.js
        const main = "main";

        //#endregion
        exports.main = main
        })(this.module = this.module || {});"
      `)
      : expect(output.output[0].code).toMatchInlineSnapshot(`
        "(function(exports) {

        "use strict";

        //#region main.js
        const main = "main";

        //#endregion
        exports.main = main
        })(this.module = this.module || {});"
      `)
  },
})
