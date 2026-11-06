import assert from 'node:assert';
import data from './data.json';

assert.strictEqual(delete data?.other, true);
assert.strictEqual(data.other, undefined);
