import assert from 'node:assert'
import("./b").then((mod) => {
  assert.strictEqual(mod.result(), "result")
})
import("./a").then(mod => {
  assert.strictEqual(mod.default, "result")
})
