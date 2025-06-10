import nodeAssert from 'node:assert'
import'./common.js'

nodeAssert.strictEqual(globalThis.value, 0)

export function render() {}
