import './dist/main.js'
import assert from "node:assert"

const modules = __rolldown_runtime__.modules;

assert.strictEqual(modules['cjs.js'].exports.value, 'cjs');