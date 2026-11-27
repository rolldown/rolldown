import assert from 'node:assert/strict';
import d from './legacy.cjs';

Object.assign(d.default, { x: 'changed' });
d.default.x ??= 'fallback';
assert.equal(d.x, 'changed');
