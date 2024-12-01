import assert from 'assert'
import('./lib.js').then((exports) => assert.deepStrictEqual({ ...exports }, {
  foo: "foo",
  bar: "bar",
  default: "default"
}));
