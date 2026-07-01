import assert from 'node:assert';
import api from './middle';

api.obj.a = 2;
assert.strictEqual(api.obj.a, 2);
