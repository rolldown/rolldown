import * as nodeFs from './main.js'
export * from 'node:fs'
import assert from 'node:assert'
assert(nodeFs.readFile instanceof Function)
