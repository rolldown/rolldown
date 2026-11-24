import assert from 'node:assert/strict';

const original = Reflect.getOwnPropertyDescriptor(globalThis, 'Object');
globalThis.calls = [];
Reflect.defineProperty(globalThis, 'Object', {
  configurable: true,
  get() {
    globalThis.calls.push('get-object');
    return original.value;
  },
});
try {
  await import('./dist/main.js');
} finally {
  Reflect.defineProperty(globalThis, 'Object', original);
}
assert.deepEqual(globalThis.calls, ['get-object', 'ready']);
