import { strict as assert } from 'node:assert';

const result = (await import('./dist/main.js')).default;
assert.deepEqual(result, ['first', 'inner-outer', 'inlined']);
