import assert from 'node:assert';
const { make, emitter } = await import('./dist/index.js');
assert.strictEqual(typeof emitter.on, 'function');
assert.strictEqual(make().constructor.name, 'EventEmitter');
