import assert from "node:assert"
import { count as originalCount } from './child'

globalThis.arrayAcceptAcceptCount ??= 0
globalThis.arrayAcceptParentExecuteCount ??= 0
globalThis.arrayAcceptParentExecuteCount++

assert.strictEqual(globalThis.arrayAcceptParentExecuteCount, 1)

let count = originalCount

import.meta.hot.accept(['./child.js'], ([mod]) => {
  count = mod.count
  globalThis.arrayAcceptAcceptCount++
  assert.strictEqual(globalThis.arrayAcceptAcceptCount, count)
})

process.on('beforeExit', (code) => {
  if (code !== 0) return
  assert.strictEqual(globalThis.arrayAcceptAcceptCount, 2)
})
