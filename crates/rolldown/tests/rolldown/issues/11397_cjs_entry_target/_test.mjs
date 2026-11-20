import assert from 'node:assert/strict';
import { load, page } from './dist/main.js';

// In this CJS build, `import()` of a CommonJS entry gives
// `__toESM(require("./target.js").default)`, so the bindings that its chunk exports to `page.js`
// are not keys of the result. An extraction made the CJS build call `n.require_target()`, which
// the chunk does not export.
const target = await load();
assert.deepEqual(Object.keys(target).sort(), ['default', 'value']);
assert.equal(target.default.value, 7);
assert.deepEqual((await page()).default(), [7, 7]);
