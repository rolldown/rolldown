import assert from "node:assert";
import * as foo from "./lib.json";

assert.deepEqual(JSON.parse(JSON.stringify(foo)).default, {
  a: 1,
  b: 2,
  c: "example",
});
