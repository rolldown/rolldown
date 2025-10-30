import assert from 'assert';
import { __default, __rest } from './dist/main';

assert.deepStrictEqual(Object.keys(__rest).sort(), ['a', 'b']);
assert.equal(__default, 'default');
