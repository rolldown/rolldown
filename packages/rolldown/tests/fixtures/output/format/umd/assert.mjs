// @ts-nocheck
import assert from 'node:assert'
import './dist/main.mjs'
assert(globalThis.module.default === 'default')
