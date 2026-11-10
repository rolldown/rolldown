import assert from 'node:assert';
import https from 'node:https';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);

const main = require('./dist/main.js');
const other = await main.loadOther();
assert.strictEqual(main.lazyNamespace(), other.eagerNamespace());
assert.strictEqual(other.eagerNamespace().default, https);
