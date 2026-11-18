import assert from 'node:assert';
import https from 'node:https';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
require('./dist/entry-a.js');
const entry = require('./dist/entry-b.js');
const originalRequest = https.request;

assert.strictEqual(entry.foo, originalRequest);

try {
  // A direct external re-export must read the current property, not capture it once.
  https.request = function replacementRequest() {};
  assert.strictEqual(entry.foo, https.request);
} finally {
  https.request = originalRequest;
}
