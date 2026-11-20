import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
assert.deepEqual(require('./dist/lib.js'), require('./lib.js'));
await import('./dist/main.js');
