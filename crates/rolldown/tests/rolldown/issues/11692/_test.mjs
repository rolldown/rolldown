import assert from 'node:assert';
import { readFileSync } from 'node:fs';

globalThis.__execOrder = [];
globalThis.__result = undefined;

const distUrl = new URL('./dist/main.js', import.meta.url);
const code = readFileSync(distUrl, 'utf8');

assert(!code.includes('__esm'), `Expected no __esm wrapper in output:\n${code}`);
assert(
  !code.includes('UNUSED_WRAPPED'),
  `Expected pure star-reexported dead target to be tree-shaken:\n${code}`,
);

await import(`${distUrl.href}?t=${Date.now()}`);

assert.strictEqual(globalThis.__result, 'item_1:a,item_2:b');
assert.deepStrictEqual(globalThis.__execOrder, ['early', 'names', 'heavy']);
