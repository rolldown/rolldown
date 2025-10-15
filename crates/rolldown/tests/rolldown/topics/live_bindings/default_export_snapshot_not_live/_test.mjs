import assert from 'node:assert';
import { default as x, inc } from './dist/main.js';
import * as star from './dist/main.js';

// The default export should be 42, captured at module evaluation time
assert.strictEqual(x, 42);
assert.strictEqual(star.default, 42);

// After incrementing, the default export should still be 42 (not a live binding)
inc();
assert.strictEqual(x, 42);
assert.strictEqual(star.default, 42);
