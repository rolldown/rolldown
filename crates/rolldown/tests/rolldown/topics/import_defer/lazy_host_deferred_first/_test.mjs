import assert from 'node:assert';
import { log, result } from './dist/main.js';

assert.deepStrictEqual(await result, ['d failed', true]);
// The load of the chunk of `d.js` does not evaluate `d.js`.
assert.deepStrictEqual(log, ['after import(importer)', 'd']);
