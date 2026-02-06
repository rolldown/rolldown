import assert from 'node:assert'
import { readFileSync } from 'node:fs'
import { value } from './dist/entry.js'

assert.strictEqual(value, 'esm-only')
const code = readFileSync(new URL('./dist/entry.js', import.meta.url), 'utf8')
assert.ok(code.includes('console.log'), 'runtime side effect should be preserved even without runtime usage')
