import { strict as assert } from 'node:assert';
import { createRequire } from 'node:module';

// Before the fix, the locals captured the lowered `import()`: `Promise.resolve is not a function`
// and `Object.freeze is not a function`. The second one throws asynchronously.
const require = createRequire(import.meta.url);
const { promiseLib, objectLib } = require('./dist/main.js');
assert.deepEqual(promiseLib.os, ['function', 'function']);
assert.equal(typeof (await promiseLib.path).sep, 'string');
assert.deepEqual(objectLib.os, ['function', 'function']);
await new Promise((resolve) => setTimeout(resolve, 50));
