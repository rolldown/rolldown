import assert from "node:assert"
import "./dist/main.js"
assert(globalThis.module.a === 1);