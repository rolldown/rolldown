import assert from 'node:assert'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const configName = globalThis.__configName

if (configName === 'esm') {
  const esm = await import('./dist/main.js')
  assert.strictEqual(Object.hasOwn(esm.lib, '__proto__'), true, 'ESM: lib.__proto__ should be an own property')
  assert.strictEqual(esm.lib['__proto__'], 123, 'ESM: lib.__proto__ should equal 123')
} else if (configName === 'cjs') {
  const cjs = require('./dist/main.cjs')
  assert.strictEqual(Object.hasOwn(cjs.lib, '__proto__'), true, 'CJS: lib.__proto__ should be an own property')
  assert.strictEqual(cjs.lib['__proto__'], 123, 'CJS: lib.__proto__ should equal 123')
} else if (configName === 'iife') {
  require('./dist/main.cjs')
  const iife = globalThis.bundle
  assert.strictEqual(Object.hasOwn(iife.lib, '__proto__'), true, 'IIFE: lib.__proto__ should be an own property')
  assert.strictEqual(iife.lib['__proto__'], 123, 'IIFE: lib.__proto__ should equal 123')
} else if (configName === 'umd') {
  const umd = require('./dist/main.cjs')
  assert.strictEqual(Object.hasOwn(umd.lib, '__proto__'), true, 'UMD: lib.__proto__ should be an own property')
  assert.strictEqual(umd.lib['__proto__'], 123, 'UMD: lib.__proto__ should equal 123')
}
