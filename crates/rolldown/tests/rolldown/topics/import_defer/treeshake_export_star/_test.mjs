import assert from 'node:assert';
import { run } from './dist/main.js';

assert.deepStrictEqual(run(), { a: 1 });
// No code uses `B`, so the static initializer of `b.js` must not run.
assert.strictEqual(globalThis.__bStaticRan, undefined);
