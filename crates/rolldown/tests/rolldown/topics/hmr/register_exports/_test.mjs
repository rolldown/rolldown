import './dist/main.js'
import assert from "node:assert"

const modules = __rolldown_runtime__.modules;

// Module IDs are now absolute paths, so we need to find the module by suffix
const cjsModule = Object.entries(modules).find(([key]) => key.endsWith('cjs.js'));
assert(cjsModule, 'cjs.js module should be registered');
assert.strictEqual(cjsModule[1].exports.value, 'cjs');