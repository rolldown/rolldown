import assert from "node:assert"
import { count as originalCount } from './child'

globalThis.singleAcceptAcceptCount ??= 0
globalThis.singleAcceptParentExecuteCount ??= 0
globalThis.singleAcceptParentExecuteCount++

assert.strictEqual(globalThis.singleAcceptParentExecuteCount, 1)

let count = originalCount

import.meta.hot.accept('./child.js', mod => {
  count = mod.count
  globalThis.singleAcceptAcceptCount++
  assert.strictEqual(globalThis.singleAcceptAcceptCount, count)
})

process.on('beforeExit', (code) => {
  if (code !== 0) return
  assert.strictEqual(globalThis.singleAcceptAcceptCount, 2)
})
