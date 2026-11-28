import assert from 'node:assert/strict';
const m = await import('./dist/entry.js');
const [x, y] = await Promise.all([m.x, m.y]);
assert.equal(x.X, 'Pq');
assert.equal(typeof y.Y, 'function');
