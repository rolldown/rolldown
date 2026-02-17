import assert from 'node:assert';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const configName = globalThis.__configName;

if (configName === 'cjs') {
  const cjs = require('./dist/main.cjs');
  assert.strictEqual(
    Object.hasOwn(cjs, '__proto__'),
    true,
    'CJS: __proto__ should be an own property',
  );
  assert.strictEqual(cjs['__proto__'], 123, 'CJS: __proto__ should equal 123');
} else if (configName === 'iife') {
  require('./dist/main.cjs');
  console.log('f', globalThis);
  const iife = globalThis.bundle;
  assert.strictEqual(
    Object.hasOwn(iife, '__proto__'),
    true,
    'IIFE: __proto__ should be an own property',
  );
  assert.strictEqual(iife['__proto__'], 123, 'IIFE: __proto__ should equal 123');
} else if (configName === 'umd') {
  const umd = require('./dist/main.cjs');
  assert.strictEqual(
    Object.hasOwn(umd, '__proto__'),
    true,
    'UMD: __proto__ should be an own property',
  );
  assert.strictEqual(umd['__proto__'], 123, 'UMD: __proto__ should equal 123');
} else {
  const esm = await import('./dist/main.js');
  assert.strictEqual(
    Object.hasOwn(esm, '__proto__'),
    true,
    'ESM: __proto__ should be an own property',
  );
  assert.strictEqual(esm['__proto__'], 123, 'ESM: __proto__ should equal 123');
}
