// @ts-nocheck
import assert from 'node:assert';
import { modules } from './dist/main';

const keys = Object.keys(modules);
assert.strictEqual(keys.length, 2, `Expected 2 modules, got ${keys.length}: ${keys.join(', ')}`);
assert.ok(
  keys.some((k) => k.endsWith('a.js')),
  'Missing a.js',
);
assert.ok(
  keys.some((k) => k.endsWith('b.js')),
  'Missing b.js',
);
