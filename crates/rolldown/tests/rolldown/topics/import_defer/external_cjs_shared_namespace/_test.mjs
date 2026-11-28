import assert from 'node:assert';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);

globalThis.__extSharedLog = [];
const main = require('./dist/main.js');
const other = require('./dist/other.js');
assert.deepStrictEqual(main.before, []);
assert.deepStrictEqual(await main.results(), {
  twoFiles: true,
  oneFile: true,
  wrapper: true,
  dynamic: true,
  foo: 'foo',
  defaultTypes: ['string', 'object'],
  log: ['ext'],
});
assert.strictEqual(other.getO(), main.getA());

delete globalThis.__extSharedLog;
