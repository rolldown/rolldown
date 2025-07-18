import assert from 'node:assert'
import fs from 'node:fs'
import path from 'node:path'
import { createRequire } from 'node:module'
const require = createRequire(import.meta.url)

switch (globalThis.__testName) {
  case undefined:
  case 'Extended Test: (minify_internal_exports: true': {
    const { foo } = await import('./dist/main.js')
    assert.strictEqual(foo, 'foo')
    break
  }
  case 'cjs': {
    const libModCjs = require('./dist/main.js')
    assert.strictEqual(libModCjs.foo, 'foo')
    break
  }
  case 'umd': {
    await import('./dist/main.js')
    assert(!!globalThis.lib)
    assert.strictEqual(globalThis.lib.foo, 'foo')
    break
  }
  case 'iife': {
    const iifeContent = fs.readFileSync(
      path.resolve(import.meta.dirname, './dist/main.js'),
      'utf-8'
    )
    ;(0, eval)(iifeContent)
    assert(!!globalThis.lib)
    assert.strictEqual(globalThis.lib.foo, 'foo')
    break
  }
  default: {
    throw new Error(`Unknown test name: ${globalThis.__testName}`)
  }
}
