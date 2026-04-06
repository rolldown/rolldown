import assert from 'node:assert';
import { createRequire } from 'node:module';

const configName = globalThis.__configName;
const require = createRequire(import.meta.url);

if (configName === 'iife' || configName === 'umd') {
  const mod = require('./dist/main.cjs');
  assert.strictEqual(mod.foo, 'foo');
} else {
  const mod = await import('./dist/main.js');
  assert.strictEqual(mod.default.foo, 'foo');
}
