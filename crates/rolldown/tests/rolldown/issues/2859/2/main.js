import assert from 'assert'

export const foo = 'foo'
export let bar = ''
bar = 'bar'
export default 'default'

import('./main.js').then((exports) => assert.deepStrictEqual({ ...exports }, {
  foo: "foo",
  bar: "bar",
  default: "default"
}));
