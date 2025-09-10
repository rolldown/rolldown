import { replacePlugin } from 'rolldown/experimental'
import { defineTest } from 'rolldown-tests'
import {expect} from 'vitest'

export default defineTest({
  config: {
    plugins: [
      replacePlugin({
        // @ts-ignore
        __rolldown_number: 42,
        // @ts-ignore
        __rolldown_string: JSON.stringify('hello world'),
        // @ts-ignore
        __rolldown_boolean_true: true,
        // @ts-ignore
        __rolldown_boolean_false: false,
        // @ts-ignore
        __rolldown_null: null,
        // @ts-ignore
        __rolldown_undefined: undefined,
        // @ts-ignore
        __rolldown_bigint: 123n,
      }),
    ],
  },
  afterTest(output) {
    const code = output.output[0].code;
    expect(code).toMatchInlineSnapshot(`
      "//#region main.js
      console.log(42);
      console.log("hello world");
      console.log(true);
      console.log(false);
      console.log(null);
      console.log(void 0);
      console.log(123);

      //#endregion"
    `)
  }
})
