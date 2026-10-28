import { strict as assert } from 'node:assert';
import { createRequire } from 'node:module';
import path from 'node:path';
const require = createRequire(import.meta.url);
assert.deepEqual(require('./dist/main.js'), [path.sep, path.sep, 'sibling']);
