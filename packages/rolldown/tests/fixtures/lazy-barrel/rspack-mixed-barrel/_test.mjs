import assert from 'node:assert';
import { c } from './dist/main.js';

// Runtime parity with rspack's `expect(c).toBe('c')`. `b` resolves through the
// explicit named re-export `export { value as b } from "./c"`, so it is c.js's
// `value` ('c'), proving the star export `export * from "./b"` was correctly
// shadowed and skipped.
assert.strictEqual(c, 'c');
