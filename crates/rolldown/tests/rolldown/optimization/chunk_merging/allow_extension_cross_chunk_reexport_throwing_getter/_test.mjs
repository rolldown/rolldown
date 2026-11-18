import assert from 'node:assert';
import { own as appOwn } from './dist/app.js';
import { foreign, own, x } from './dist/seg.js';

// Code outside the bundle imports from the facade of `seg.js`. The getter of `x` throws. The
// facade still loads, and `x` is `undefined`, as in Node.
assert.strictEqual(own, 17);
assert.strictEqual(foreign, 5);
assert.strictEqual(x, undefined);
assert.strictEqual(appOwn, 17);
