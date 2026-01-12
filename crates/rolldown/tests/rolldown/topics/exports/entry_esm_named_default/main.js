import assert from 'node:assert';
import lib from './lib.js';

// Access default export from ESM entry chunk with Named OutputExports
assert.strictEqual(lib.value, 42);

export { lib };
