import assert from 'node:assert';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);

globalThis.__extLog = [];
const main = require('./dist/main.js');
assert.deepStrictEqual(main.readBarrel(), [42, 7]);
assert.deepStrictEqual(main.readDeferred(), [42, 7]);
assert.deepStrictEqual(globalThis.__extLog, ['ext-barrel', 'ext-barrel-star']);

delete globalThis.__extLog;
