import assert from 'node:assert/strict';

let amdCalls = 0;
globalThis.define = Object.assign(
  () => {
    amdCalls += 1;
  },
  { amd: true },
);

await import(`./dist/main.js?${Date.now()}`);

assert.equal(globalThis.result, 'hello from umd dep');
assert.equal(amdCalls, 0);
