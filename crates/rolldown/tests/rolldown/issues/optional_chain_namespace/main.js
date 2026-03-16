import assert from 'node:assert';
import * as h from './state.js';
// Optional chaining on namespace member values must be preserved
assert.strictEqual(h.app?.user?.name ?? 'ok', 'ok');
assert.strictEqual(h['app']?.['user']?.['name'] ?? 'ok', 'ok');
