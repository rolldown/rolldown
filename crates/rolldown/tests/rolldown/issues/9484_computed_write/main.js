import assert from 'node:assert';
import data from './data.json';

const key = 'search';
data[key] = 'overridden';
assert.strictEqual(data.search, 'overridden');
