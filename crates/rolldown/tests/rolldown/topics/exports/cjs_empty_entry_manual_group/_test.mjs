import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
assert.deepStrictEqual(require('./dist/empty.js'), {});
