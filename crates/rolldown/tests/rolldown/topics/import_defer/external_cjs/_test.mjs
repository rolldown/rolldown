import assert from 'node:assert';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);

globalThis.__extLog = [];
const main = require('./dist/main.js');
assert.deepStrictEqual(globalThis.__extLog, ['main']);

// `import.defer()` does not load the external.
const dynamic = await main.load();
assert.strictEqual(Object.prototype.toString.call(dynamic), '[object Deferred Module]');
assert.deepStrictEqual(globalThis.__extLog, ['main']);

// An escape does not load it either.
const escaped = main.escape();
assert.strictEqual(Object.prototype.toString.call(escaped), '[object Deferred Module]');
assert.deepStrictEqual(globalThis.__extLog, ['main']);

// The first access to a key loads it, one time.
assert.strictEqual(main.hasValue(), true);
assert.deepStrictEqual(globalThis.__extLog, ['main', 'ext-lazy']);
assert.strictEqual(main.read(), 42);
assert.strictEqual(escaped.value, 42);
assert.strictEqual(dynamic.value, 42);
assert.strictEqual(escaped.then, undefined);
assert.deepStrictEqual(globalThis.__extLog, ['main', 'ext-lazy']);

delete globalThis.__extLog;
