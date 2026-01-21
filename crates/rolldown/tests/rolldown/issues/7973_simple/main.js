import cjs from "./cjs.js";
import assert from "node:assert";

// This should work - it's a read
assert.strictEqual(cjs.a, "original");

// This is the bug - assignment to property on default import from CJS
// Without the fix, this throws: Cannot set property a of #<Object> which has only a getter
cjs.a = "new value";

assert.strictEqual(cjs.a, "new value");
