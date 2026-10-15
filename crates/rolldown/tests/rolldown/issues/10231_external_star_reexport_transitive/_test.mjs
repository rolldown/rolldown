import { createRequire } from 'node:module';
import assert from 'node:assert';

const require = createRequire(import.meta.url);
const mid = require('./dist/mid.js');

assert.strictEqual(require('./dist/entry.js').seen, 'value');
assert.strictEqual(mid.default.name, 'value');
assert.strictEqual(
  typeof mid.join,
  'function',
  'transitive external star re-export must stay reachable',
);
