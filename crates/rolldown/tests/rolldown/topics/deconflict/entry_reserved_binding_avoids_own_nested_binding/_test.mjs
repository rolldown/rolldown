import { strict as assert } from 'node:assert';
import { createRequire } from 'node:module';

// Pre-fix, the renamed top-level `exports` landed on `f`'s parameter `exports$1`, so `f` returned
// `['param', 'param']`, with no error at all.
const { f } = createRequire(import.meta.url)('./dist/main.js');

assert.deepEqual(f('param'), ['local-exports', 'param']);
