import assert from 'node:assert';
import api, { used } from './middle';

assert.strictEqual(api.used, 'used');
assert.strictEqual(used, 'used');
