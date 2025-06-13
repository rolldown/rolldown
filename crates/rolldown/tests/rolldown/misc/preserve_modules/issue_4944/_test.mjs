import { assert } from "node:test";
import { b } from "./dist/a/index.js";

assert.strictEqual(b, 2, "b should be 2");
