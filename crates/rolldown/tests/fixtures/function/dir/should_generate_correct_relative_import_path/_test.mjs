import assert from 'node:assert';
import { value, asyncValue } from './dist/lib/entries/a.mjs'
import { value as value2, asyncValue as asyncValue2 } from './dist/lib/entries/b.mjs'

assert.strictEqual(value, value2);
assert.strictEqual(await asyncValue, await asyncValue2);