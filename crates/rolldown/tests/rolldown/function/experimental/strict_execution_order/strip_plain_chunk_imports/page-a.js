import nodeAssert from 'node:assert'
import { common, _ } from './common.js'

nodeAssert.strictEqual(globalThis.value, 0)

export function render() {
  console.log(common, _)
}
