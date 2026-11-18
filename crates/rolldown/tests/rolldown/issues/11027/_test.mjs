import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const raw = require('./lib.cjs');
const built = require('./dist/lib.js');
assert.deepEqual({ ...built, shared: built.shared() }, { ...raw, shared: raw.shared() });
assert.equal(Object.getPrototypeOf(built), Object.getPrototypeOf(raw));
await import('./dist/main.js');
