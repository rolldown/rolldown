import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
assert.deepEqual({ ...require('./dist/proxy.js') }, { x: 1 });
assert.deepEqual({ ...require('./dist/entry.js') }, { x: 1 });
