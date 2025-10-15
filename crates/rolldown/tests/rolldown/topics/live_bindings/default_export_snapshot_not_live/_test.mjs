import assert from 'node:assert';
import { file } from './dist/main.js';

// The default export should be 42, captured at module evaluation time
assert.strictEqual(file.default, 42);

// After incrementing, the default export should still be 42 (not a live binding)
file.inc();
assert.strictEqual(file.default, 42);
