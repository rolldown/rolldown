import assert from 'node:assert';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);

globalThis.__extLog = [];
const main = require('./dist/main.js');
assert.deepStrictEqual(globalThis.__extLog, ['main']);
assert.strictEqual(main.read(), 7);
assert.deepStrictEqual(globalThis.__extLog, ['main', 'ext-ns']);

const consumer = await main.loadConsumer();
assert.strictEqual(consumer.consumerRead(), 7);
assert.strictEqual(consumer.consumerNs, main.escape());
assert.deepStrictEqual(globalThis.__extLog, ['main', 'ext-ns']);

delete globalThis.__extLog;
