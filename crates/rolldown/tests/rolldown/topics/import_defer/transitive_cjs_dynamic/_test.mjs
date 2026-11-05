import assert from 'node:assert';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);

globalThis.__extLog = [];
const main = require('./dist/main.js');
const ns = await main.load();
assert.deepStrictEqual(globalThis.__extLog, ['main']);
assert.strictEqual(ns.x, 5);
assert.deepStrictEqual(globalThis.__extLog, ['main', 'ext-dyn']);

delete globalThis.__extLog;
