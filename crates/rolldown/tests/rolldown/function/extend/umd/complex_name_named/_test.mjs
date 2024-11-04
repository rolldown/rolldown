import assert from "node:assert"
import "./dist/main.js"
assert(globalThis.test.module.a === 1);
