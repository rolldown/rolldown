import assert from 'node:assert';
import { log, values } from './dist/main.js';

assert.deepStrictEqual(values, ['d', 'e']);
// Each module is in its own chunk. A chunk load does not evaluate a deferred module.
assert.deepStrictEqual(log, ['main', 'after import.defer', 'd', 'e']);
