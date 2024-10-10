import assert from 'node:assert';
import { value, asyncValue } from './dist/lib/entries/a.js'
import { value as value2, asyncValue as asyncValue2 } from './dist/lib/entries/b.js'

assert.strictEqual(value, value2);
assert.strictEqual(await asyncValue, await asyncValue2);
