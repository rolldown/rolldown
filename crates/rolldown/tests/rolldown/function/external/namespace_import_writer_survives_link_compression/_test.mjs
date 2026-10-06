import assert from 'node:assert';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { readDefault } = require('./dist/entry.js');
const raw = require('external-pkg');

// `shim.mjs` is ESM, so Node hands it the raw `module.exports` as `default`,
// regardless of the `__esModule` marker the package sets.
assert.strictEqual(
  readDefault(),
  raw,
  `expected the raw module.exports (marker=${raw.marker}), got ${JSON.stringify(readDefault())}`,
);
