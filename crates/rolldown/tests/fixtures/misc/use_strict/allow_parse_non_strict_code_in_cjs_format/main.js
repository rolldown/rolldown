import assert from 'node:assert'
import foo from './cjs'
assert(typeof foo === 'function')
export {}
