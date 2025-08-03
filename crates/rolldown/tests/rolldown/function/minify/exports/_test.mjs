import assert from 'node:assert'
import fs from 'node:fs'
import path from 'node:path'
import { createRequire } from 'node:module'
const require = createRequire(import.meta.url)

switch (globalThis.__configName) {
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
    if (!globalThis.__configName || globalThis.__configName.startsWith('extended')) {
      const { foo } = await import('./dist/main.js')
      assert.strictEqual(foo, 'foo')
    } else {
      throw new Error(`Unknown test name: ${globalThis.__configName}`)
    }
  }
}
