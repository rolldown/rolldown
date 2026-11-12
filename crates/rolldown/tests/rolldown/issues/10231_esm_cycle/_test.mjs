import { createRequire } from 'node:module';
import assert from 'node:assert';
const require = createRequire(import.meta.url);
assert.strictEqual(require('./dist/index.js')(), 'A');
