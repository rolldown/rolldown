import assert from 'node:assert';
import { b } from './dist/main.js';

// Runtime parity with rspack's `expect(b).toBe('b')`. `b` is not an explicit
// named re-export, so it is resolved by searching the star re-exports
// (`export * from "./a"`, `export * from "./b"`); it comes from b.js.
assert.strictEqual(b, 'b');
