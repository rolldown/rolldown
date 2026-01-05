import './dist/main.js';
import assert from 'node:assert';

const modules = __rolldown_runtime__.modules;

// Module IDs are stable paths (relative to cwd)
const cjsModule = Object.entries(modules).find(([key]) => key === 'cjs.js');
assert(cjsModule, 'cjs.js module should be registered');
assert.strictEqual(cjsModule[1].exports.value, 'cjs');
