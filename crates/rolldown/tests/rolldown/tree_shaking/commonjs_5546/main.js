import assert from 'node:assert'
import mod from './lib';

let indirect = mod;

assert.strictEqual(indirect.default, 'default');
assert.strictEqual(indirect.foo, 'foo');
