import assert from "node:assert"
import {bar as a} from "./foo.js"
import("./foo.js").then(({default: {bar: b}}) => {
  assert.equal(a, 123);
  assert.equal(b, 123);
})
