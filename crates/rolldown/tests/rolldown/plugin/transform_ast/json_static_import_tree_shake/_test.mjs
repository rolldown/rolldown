import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { used } from './dist/main.js';

assert.equal(globalThis.jsonTreeShakeSideEffectRan, true);
assert.equal(used, 1);

const output = await readFile(new URL('./dist/main.js', import.meta.url), 'utf8');
assert.match(output, /\.used/);
assert.doesNotMatch(output, /\bvar unused\b|\.unused/);
