import assert from 'node:assert';
import { nested } from './dist/main.js';

// Runtime parity with rspack's `expect(nested.a).toBe('b')`.
// nested-barrel/a.js does `import { b as a } from "./b"; export { a }`, so the
// namespace's `a` is b.js's `b` ('b'). `c` comes from c.js ('c').
assert.strictEqual(nested.a, 'b');
assert.strictEqual(nested.c, 'c');
