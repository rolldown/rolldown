import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

await import('./dist/main.js');

const output = await readFile(new URL('./dist/main.js', import.meta.url), 'utf8');
const payload = output.indexOf('var data_default');
const initializer = output.indexOf('var old = data_default.old');
const appendedEffect = output.indexOf('globalThis.jsonAppendedExpressionRan = true');

assert.ok(payload >= 0);
assert.ok(initializer > payload);
assert.ok(appendedEffect >= 0);
assert.ok(payload > appendedEffect);
