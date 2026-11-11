import assert from 'node:assert/strict';
import * as path from 'node:path';
import { barrel, fromInner, sep } from './dist/main.js';

// In the generate stage, `barrel.js` loses its dynamic exports. But the output still has a
// `__reExport` that adds the exports of `node:path` to its namespace object. So the object stays
// open, and the bundle loads with no `TypeError`.
assert.equal(Object.getPrototypeOf(barrel), null);
assert.equal(barrel.sep, path.sep);
assert.equal(barrel.fromInner, 'inner');
assert.equal(fromInner, 'inner');
assert.equal(sep, path.sep);
