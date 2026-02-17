import assert from 'node:assert'
import { a } from './a'

globalThis.acceptOutsideCircularExecCount ??= 0
globalThis.acceptOutsideCircularExecCount++

if (globalThis.acceptOutsideCircularExecCount === 1) {
  assert.strictEqual(a.b.c, 'c')
} else {
  assert.strictEqual(a.b.c, 'cc')
}

import.meta.hot.accept(() => {})
