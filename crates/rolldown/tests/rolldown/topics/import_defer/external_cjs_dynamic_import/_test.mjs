import assert from 'node:assert';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);

globalThis.__extDynLog = [];
const main = require('./dist/main.js');
assert.deepStrictEqual(globalThis.__extDynLog, ['main']);

// `import()` evaluates the external when it loads, as in the build with the option off.
const cjs = await main.loadCjs();
assert.deepStrictEqual(globalThis.__extDynLog, ['main', 'ext-cjs']);
assert.strictEqual(cjs.value, 1);

// `require()` throws for an ES module with top-level await, but `import()` loads it.
const tla = await main.loadTla();
assert.deepStrictEqual(globalThis.__extDynLog, ['main', 'ext-cjs', 'ext-tla']);
assert.strictEqual(tla.value, 2);

delete globalThis.__extDynLog;
