import assert from 'node:assert/strict';
import { check, Status } from './param';
import { Color, inner } from './nested';
import { A, B } from './namespace';

assert.strictEqual(check({ Ok: 'fine' }), 'fine');
assert.strictEqual(Status.Ok, 1);
assert.strictEqual(Color.Red, 'red');
assert.strictEqual(inner(), 5);
assert.strictEqual(A.E.X, 1);
assert.strictEqual(B.value, 'b');
assert.deepStrictEqual(B.object, { X: 'b' });
