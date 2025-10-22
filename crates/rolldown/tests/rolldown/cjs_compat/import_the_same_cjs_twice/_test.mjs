import assert from 'node:assert'
import { js_a, js_a2, js_def, js_def2, mjs_a, mjs_a2, mjs_def, mjs_def2 } from './dist/main.js'

assert.strictEqual(js_a, js_a2)
assert.strictEqual(js_def, js_def2)

assert.strictEqual(mjs_a, mjs_a2)
assert.strictEqual(mjs_def, mjs_def2)
