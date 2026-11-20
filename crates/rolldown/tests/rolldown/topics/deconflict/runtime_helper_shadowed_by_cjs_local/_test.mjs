import { strict as assert } from 'node:assert';

const { default: lib } = await import('./dist/main.js');

assert.deepEqual(lib, { v: 1, local: 'local' });
