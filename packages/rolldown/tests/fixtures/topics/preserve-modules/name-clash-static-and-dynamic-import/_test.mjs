import assert from 'node:assert';
import value, { dynamic } from './dist/main.mjs';

assert.strictEqual(value, 'static:static');
assert.strictEqual(await dynamic, 'dynamic:dynamic');
assert.deepStrictEqual(globalThis.nameClashStaticAndDynamicImport, ['static', 'dynamic']);
