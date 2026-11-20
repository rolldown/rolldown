import assert from 'node:assert/strict';

const namespace = await import('./re.js');
assert.ok(Object.keys(namespace).includes('foo'));
assert.equal(typeof namespace.foo, 'function');
