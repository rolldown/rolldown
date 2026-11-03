import assert from 'node:assert';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
require('./dist/main.js');
assert.strictEqual(globalThis.afterReturn, undefined);
assert.strictEqual(globalThis.outroRan, true);
assert.strictEqual(globalThis.footerRan, true);
