import assert from 'node:assert';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);

// Code outside the bundle loads the facade of `seg.js`. A CJS file must load through `require()`:
// for `import()`, Node reads each export of a CommonJS module one time, and ignores a throw.
const [app, seg] =
  globalThis.__configName === 'cjs'
    ? [require('./dist/app.js'), require('./dist/seg.js')]
    : [await import('./dist/app.js'), await import('./dist/seg.js')];

assert.strictEqual(seg.own, 17);
assert.strictEqual(seg.foreign, 5);
assert.strictEqual(app.own, 17);
// The getter of `x` throws. The facade still loads, and `x` is `undefined`, as in Node.
assert.strictEqual(seg.x, undefined);
// The getter of `n` counts its reads. Node reads `n` one time, so both aliases are `1`.
assert.strictEqual(seg.first, 1);
assert.strictEqual(seg.second, 1);
assert.strictEqual(seg.first, 1);
const copy = { ...seg };
assert.strictEqual(copy.x, undefined);
assert.strictEqual(copy.first, 1);
assert.strictEqual(copy.second, 1);
