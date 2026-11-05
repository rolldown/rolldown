import assert from 'node:assert';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);

globalThis.__extLog = [];
const main = require('./dist/main.js');
assert.deepStrictEqual(main.same(), [true, true, 7]);
assert.deepStrictEqual(globalThis.__extLog, ['ext-ns', 'main']);

delete globalThis.__extLog;
