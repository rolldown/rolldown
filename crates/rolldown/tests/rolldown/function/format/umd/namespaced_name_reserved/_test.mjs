import assert from "node:assert"
import "./dist/main.js"
assert(typeof globalThis["1"]["2"] === "object");
