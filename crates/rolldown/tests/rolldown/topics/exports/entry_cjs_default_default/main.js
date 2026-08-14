import assert from 'node:assert';
import lib from './lib.js';

// Access default export from CJS entry chunk
assert.strictEqual(lib.value, 42);

// Re-export as default to satisfy exports: "default" requirement
export default lib;
