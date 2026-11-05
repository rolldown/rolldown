import assert from 'node:assert/strict';
import { hyphenated, quoted, backslash, lineBreak } from './dist/main.js';

assert.deepStrictEqual(hyphenated, { './dir/index.js': 1 });
assert.deepStrictEqual(quoted, { './dir/index.js': 2 });
assert.deepStrictEqual(backslash, { './dir/index.js': 3 });
assert.deepStrictEqual(lineBreak, { './dir/index.js': 4 });
