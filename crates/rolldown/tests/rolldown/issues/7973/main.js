import cjs from "./cjs.js";
import assert from "node:assert";
import { name } from './flag.js'

// This should work - it's a read
assert.strictEqual(cjs.a, "original");
assert.strictEqual(cjs.b, "original");

// Static member expression assignment (cjs.a = ...)
// Without the fix, this throws: Cannot set property a of #<Object> which has only a getter
cjs.a = "new value a";
assert.strictEqual(cjs.a, "new value a");

// Computed member expression assignment (cjs["b"] = ...)
cjs[name] = "new value b";
assert.strictEqual(cjs["b"], "new value b");
