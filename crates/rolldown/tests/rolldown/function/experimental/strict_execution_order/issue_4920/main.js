import nodeAssert from "node:assert"

import dep from "./dep.js"

nodeAssert.strictEqual(globalThis.value, 1)

export { dep }