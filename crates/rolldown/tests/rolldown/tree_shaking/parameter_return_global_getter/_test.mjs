import assert from 'node:assert/strict';

globalThis.calls = [];
Object.defineProperty(globalThis, 'window', {
  configurable: true,
  get() {
    globalThis.calls.push('window shim');
    return {};
  },
});
await import('./dist/main.js');
delete globalThis.window;
assert.deepEqual(globalThis.calls, ['window shim', 'ready']);
