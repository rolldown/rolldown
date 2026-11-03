import assert from 'node:assert';
import './a.js';
import * as b from './b.js';

assert.strictEqual('missing' in b, false);
assert.strictEqual(b.missing, undefined);
