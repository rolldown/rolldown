import mod from "./commonjs.js";
import assert from "node:assert";


// Don't optimize those cases
assert.equal(mod.foo, 1);
assert.equal(mod.bar, 2);
