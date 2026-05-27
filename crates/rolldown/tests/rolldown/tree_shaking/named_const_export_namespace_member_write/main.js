import assert from 'node:assert';
import { res } from './middle';

res.obj.a = 2;
assert.strictEqual(res.obj.a, 2);
