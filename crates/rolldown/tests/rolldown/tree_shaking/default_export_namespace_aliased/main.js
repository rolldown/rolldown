import assert from 'node:assert';
import api, { foo } from './middle';

assert.strictEqual(api.used, 'used');
assert.strictEqual(foo.used, 'used');
