import array from './data.json';
import string from './string.json';
import number from './number.json';
import boolean from './boolean.json';
import nullValue from './null.json';
import text from './text.data';

import assert from 'assert';

assert.deepStrictEqual(array, [1, 2, 3]);
assert.strictEqual(string, 'hello world');
assert.strictEqual(number, 42);
assert.strictEqual(boolean, true);
assert.strictEqual(nullValue, null);
assert.strictEqual(text, 'hello from text\n');
