import { strict as assert } from 'node:assert';

// Pre-fix, `__toCommonJS(esm_exports)` called the local: `{ v: undefined, local: 'local' }`.
const { default: lib } = await import('./dist/main.js');

assert.deepEqual(lib, { v: 1, local: 'local' });
