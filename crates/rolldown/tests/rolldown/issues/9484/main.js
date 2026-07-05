import assert from 'node:assert';
import data from './data.json';

data.search = 'overridden';
assert.strictEqual(data.search, 'overridden');
