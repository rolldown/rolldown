import assert from "node:assert";
import mod from "./foo.json";
import mod2 from "./bailout.json";

assert.strictEqual(mod.a, "used");
assert.strictEqual(mod2.a, "bailout_a");
assert.deepEqual(mod2, {
  a: "bailout_a",
  b: "bailout_b",
});
