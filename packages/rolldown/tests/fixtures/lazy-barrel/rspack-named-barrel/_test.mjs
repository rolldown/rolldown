import assert from 'node:assert';
import { a, cc } from './dist/main.js';

// Runtime parity with rspack's `expect(a).toBe('a')` / `expect(cc).toBe('c')`.
// Proves the lazy-barrel skip (b.js + d.js not loaded) did not corrupt the
// values that ARE used.
assert.strictEqual(a, 'a');
assert.strictEqual(cc, 'c');
