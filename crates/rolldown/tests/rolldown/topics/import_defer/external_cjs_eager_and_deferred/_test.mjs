import assert from 'node:assert';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);

globalThis.__extLog = [];
const main = require('./dist/main.js');
// The eager import loads the external first.
assert.deepStrictEqual(globalThis.__extLog, ['ext-lazy', 'main']);
assert.deepStrictEqual(main.read(), [42, 42]);
assert.deepStrictEqual(globalThis.__extLog, ['ext-lazy', 'main']);

delete globalThis.__extLog;
