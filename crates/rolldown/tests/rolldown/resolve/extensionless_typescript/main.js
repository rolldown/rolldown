import assert from 'node:assert'
import { valueFromTs } from './module.js'  // .js extension resolves to .ts file
import { utilFromMts } from './util.mjs'   // .mjs extension resolves to .mts file

// Test that .js resolves to .ts
assert.strictEqual(valueFromTs, 'typescript-value')

// Test that .mjs resolves to .mts  
assert.strictEqual(utilFromMts, 'mts-value')
