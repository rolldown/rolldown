import assert from "node:assert";
import "./foo";
import "./dir/index.js";

assert.strictEqual(globalThis.foo, undefined);
