import './dist/main.js';
import assert from 'node:assert';

const moduleCache = __rolldown_runtime__.moduleCache;

// Module IDs are stable paths (relative to cwd)
const cjsModule = moduleCache.get('cjs.js');
assert(cjsModule, 'cjs.js module should be registered');
assert.strictEqual(cjsModule.exports.value, 'cjs');
