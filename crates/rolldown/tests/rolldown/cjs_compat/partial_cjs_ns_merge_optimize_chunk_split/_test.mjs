import assert from "assert";
import { aa, bb } from "./dist/main.js";
import { entry } from "./dist/entry.js";

assert.strictEqual(aa, 10000);
assert.strictEqual(bb, 10000);
assert.strictEqual(entry, 10000);
