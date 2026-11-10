import assert from 'node:assert';
import { readFileSync } from 'node:fs';
import { run } from './dist/main.js';

assert.deepStrictEqual(run(), ['helper m', true]);
assert.deepStrictEqual(globalThis.__log, ['m']);
const code = readFileSync(new URL('./dist/main.js', import.meta.url), 'utf8');
for (const text of [
  'legal before a removed import',
  'legal before a hoisted function',
  'legal in the body',
]) {
  assert.strictEqual(code.split(text).length - 1, 1, text);
}
